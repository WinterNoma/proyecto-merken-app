#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod inner {
    use crate::{
        control::{CancelToken, Control, Deadline, Progress},
        engine::{DEFAULT_SEED, DEFAULT_TEMPERATURE, MockEngine},
        error::SlmError,
        explain::{
            AI_SUGGESTION_LABEL, Document, ERR_FORBIDDEN_NEGATION, ERR_FORBIDDEN_NUMBER,
            ERR_INCOMPATIBLE_STATUS, ERR_MALFORMED_CITATION, ERR_NON_ENGLISH,
            ERR_OUT_OF_VOCABULARY, ERR_SENTENCE_COUNT, ERR_SENTENCE_TOO_LONG, ERR_TOO_LONG,
            ERR_UNKNOWN_CITATION, ERR_UNVERIFIED_APPROVAL, Explainer, FactId, FactLine,
            PromptFormat, SlmExplainer, render_facts, validate_note,
        },
    };
    use merken_core::{
        RULES_VERSION, VERSION,
        catalog::BodyZone,
        evaluate::{Direction, Escalation, Evaluation, NotEvaluatedReason, SourceKind, Status},
        time::Timestamp,
    };
    use serde_json::json;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };

    fn sample_eval(status: Status, escalation: Escalation, source_kind: SourceKind) -> Evaluation {
        Evaluation {
            indicator: "heart_rate".into(),
            body_zone: None,
            status,
            reason: None,
            direction: None,
            persistent: false,
            escalation,
            rule_id: "heart_rate.personal_change".into(),
            rule_version: RULES_VERSION.into(),
            source_kind,
            source_ref: Some("AHA 2020 Guidelines (>= 150/min)".into()),
            input_record_ids: vec!["r1".into()],
            input_event_ids: vec![],
            core_version: VERSION.into(),
            evaluated_at: Timestamp::from_millis(1_763_154_333_120),
            latest_value: Some(json!({ "bpm": 72 })),
            latest_measured_at: Some(Timestamp::from_millis(1_763_154_333_120)),
            baseline: Some(70.0),
            history_count: 5,
            history_needed: 3,
            summary: "Within your recent range".into(),
        }
    }

    fn docs() -> Vec<Document> {
        vec![
            Document {
                id: "hr-rest-1".into(),
                text:
                    "Compare resting pulse against your personal baseline after 5 minutes of rest."
                        .into(),
            },
            Document {
                id: "cmo-alarm".into(),
                text: "Notify the onboard CMO and open the offline protocol.".into(),
            },
        ]
    }

    fn explain_with_output(
        eval: &Evaluation,
        docs: &[Document],
        raw_output: &str,
    ) -> crate::explain::Explanation {
        let engine = MockEngine::with_responses([raw_output]).with_model_id("space-health-v5");
        let mut explainer = SlmExplainer::with_engine(engine);
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));
        explainer.explain(eval, docs, &mut ctl).unwrap()
    }

    // --- Pruebas obligatorias contra todos los P0 de reviews/slm-f0-crate.md ---

    #[test]
    fn rejects_all_sol_p0_examples_with_note_none() {
        let stable_demo = sample_eval(
            Status::Stable,
            Escalation::Continue,
            SourceKind::DemoHeuristic,
        );
        let alarm_demo = sample_eval(Status::Alarm, Escalation::Urgent, SourceKind::DemoHeuristic);
        let mut not_eval = sample_eval(
            Status::NotEvaluated,
            Escalation::Continue,
            SourceKind::DemoHeuristic,
        );
        not_eval.reason = Some(NotEvaluatedReason::NoData);
        not_eval.latest_value = None;
        not_eval.latest_measured_at = None;
        not_eval.baseline = None;

        // 1. Diagnóstico y tratamiento sin documentos
        let exp = explain_with_output(
            &stable_demo,
            &[],
            "You have a blood clot; take two aspirin every morning.",
        );
        assert!(exp.note.is_none());
        assert!(exp.note_discarded.is_some());
        assert!(!exp.facts.is_empty());

        // 2. Contradicción en alarma / urgente
        let exp = explain_with_output(
            &alarm_demo,
            &docs(),
            "You are safe; contacting the CMO is unnecessary.",
        );
        assert!(exp.note.is_none());
        assert!(exp.note_discarded.is_some());

        // 3. Afirmación de estado en not_evaluated
        let exp = explain_with_output(&not_eval, &docs(), "Your health is excellent.");
        assert!(exp.note.is_none());
        assert!(exp.note_discarded.is_some());

        // 4. Escalado de alarma estando estable
        let exp = explain_with_output(
            &stable_demo,
            &docs(),
            "Contact the onboard CMO immediately.",
        );
        assert!(exp.note.is_none());
        assert!(exp.note_discarded.is_some());

        // 5. Lectura y referencia intercambiadas
        let exp = explain_with_output(
            &stable_demo,
            &docs(),
            "Your latest pulse is 70 bpm and your baseline is 72.0 bpm.",
        );
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_FORBIDDEN_NUMBER));

        // 6. Número en palabras en inglés
        let exp = explain_with_output(
            &stable_demo,
            &docs(),
            "Your pulse is ninety-nine beats per minute.",
        );
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_FORBIDDEN_NUMBER));

        // 7. Porcentaje con ficha ajena de humedad
        let mut docs_with_humidity = docs();
        docs_with_humidity.push(Document {
            id: "env-humidity".into(),
            text: "Humidity is 40%.".into(),
        });
        let exp = explain_with_output(
            &stable_demo,
            &docs_with_humidity,
            "Your pulse fell by 72% compared with baseline.",
        );
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_FORBIDDEN_NUMBER));

        // 8. Porcentaje en palabras ("seventy-two per cent")
        let exp = explain_with_output(
            &stable_demo,
            &docs(),
            "Your pulse dropped by seventy-two per cent.",
        );
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_FORBIDDEN_NUMBER));

        // 9. Fuente inventada sin documentos
        let exp = explain_with_output(
            &stable_demo,
            &[],
            "The NASA Unicorn Manual confirms that you are safe.",
        );
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_OUT_OF_VOCABULARY));

        // 10. Cita real de reposo usada para recomendar aspirina
        let exp = explain_with_output(&stable_demo, &docs(), "Take aspirin [hr-rest-1].");
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_OUT_OF_VOCABULARY));

        // 11. Aprobación inventada con demo_heuristic
        let exp = explain_with_output(
            &stable_demo,
            &docs(),
            "This protocol has been approved for flight.",
        );
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_UNVERIFIED_APPROVAL));

        // 12. Mezcla inglés + español
        let exp = explain_with_output(
            &stable_demo,
            &docs(),
            "Your reading is available; espera tranquilamente.",
        );
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_OUT_OF_VOCABULARY));

        // 13. Salida completa en español (con y sin tildes)
        let exp_es_accents = explain_with_output(
            &stable_demo,
            &docs(),
            "Tu frecuencia cardíaca está dentro de tu rango reciente.",
        );
        assert!(exp_es_accents.note.is_none());
        assert_eq!(exp_es_accents.note_discarded, Some(ERR_NON_ENGLISH));

        let exp_es_plain = explain_with_output(
            &stable_demo,
            &docs(),
            "Tu pulso se mantiene estable dentro de tu rango reciente.",
        );
        assert!(exp_es_plain.note.is_none());
        assert_eq!(exp_es_plain.note_discarded, Some(ERR_OUT_OF_VOCABULARY));
    }

    // --- Notas válidas que sí superan el vocabulario cerrado ---

    #[test]
    fn accepts_paraphrase_of_facts_and_cited_document_suggestion() {
        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let facts = render_facts(&eval);

        // Paráfrasis pura de los hechos (sin números, sin citas)
        let paraphrase =
            "Your heart rate is stable within your recent range. Continue routine monitoring.";
        let note = validate_note(paraphrase, &facts, &docs()).unwrap();
        assert_eq!(note.text, paraphrase);
        assert_eq!(note.label, AI_SUGGESTION_LABEL);
        assert!(note.cited_docs.is_empty());

        // Sugerencia que cita una ficha y usa sus palabras + sufijos simples (-ing / -ed)
        let cited = "Your heart rate is within your recent range. Compare your resting pulse against your personal baseline after rest [hr-rest-1].";
        let exp = explain_with_output(&eval, &docs(), cited);
        let ai_note = exp.note.expect("valid cited suggestion must pass");
        assert!(exp.note_discarded.is_none());
        assert_eq!(ai_note.label, AI_SUGGESTION_LABEL);
        assert_eq!(ai_note.cited_docs, vec!["hr-rest-1".to_owned()]);
        assert_eq!(exp.model_id, "space-health-v5");

        // Variante morfológica con sufijo simple (-s / -ly)
        let suffixed = "Your heart rate continues within your recent range, comparing resting pulses against your personal baselines [hr-rest-1].";
        assert!(validate_note(suffixed, &facts, &docs()).is_ok());
    }

    #[test]
    fn allows_approved_protocol_claim_only_when_source_kind_is_approved_and_in_facts() {
        let eval_approved = sample_eval(
            Status::Alarm,
            Escalation::Urgent,
            SourceKind::ApprovedProtocol,
        );
        let facts_approved = render_facts(&eval_approved);
        let note_text = "Your heart rate is in alarm under the approved protocol. Notify the onboard CMO and open the offline protocol [cmo-alarm].";
        let note = validate_note(note_text, &facts_approved, &docs()).unwrap();
        assert_eq!(note.cited_docs, vec!["cmo-alarm".to_owned()]);

        // Aunque una ficha mencione "approved" o "certified", con Published o Demo se rechaza
        let doc_with_approval = vec![Document {
            id: "doc-app".into(),
            text: "Follow the approved and certified resting guidance.".into(),
        }];
        let eval_published =
            sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let facts_published = render_facts(&eval_published);
        assert_eq!(
            validate_note(
                "Follow the approved guidance [doc-app].",
                &facts_published,
                &doc_with_approval
            ),
            Err(ERR_UNVERIFIED_APPROVAL)
        );

        // Incluso con ApprovedProtocol, "certified" no está en los hechos -> rechazado
        assert_eq!(
            validate_note(
                "This guidance is certified [doc-app].",
                &facts_approved,
                &doc_with_approval
            ),
            Err(ERR_UNVERIFIED_APPROVAL)
        );
    }

    // --- Cobertura de render_facts ---

    #[test]
    fn render_facts_covers_all_statuses_reasons_escalations_and_sources() {
        // 1. Stable + Continue + Published
        let stable = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let f_stable = render_facts(&stable);
        assert!(f_stable.iter().any(|f| {
            f.id == FactId::Status && f.text.contains("Stable") && f.text.contains("recent range")
        }));
        assert!(f_stable.iter().any(|f| {
            f.id == FactId::LatestReading
                && f.text.contains("72 bpm")
                && f.text.contains("2025-11-14T")
        }));
        assert!(
            f_stable
                .iter()
                .any(|f| f.id == FactId::Baseline && f.text.contains("70 bpm"))
        );
        assert!(
            f_stable
                .iter()
                .any(|f| f.id == FactId::History && f.text.contains("5/3"))
        );
        assert!(
            f_stable
                .iter()
                .any(|f| f.id == FactId::Escalation
                    && f.text.contains("continue routine monitoring"))
        );
        assert!(f_stable.iter().any(|f| {
            f.id == FactId::Source
                && f.text.contains("published evidence")
                && f.text.contains("AHA 2020")
                && !f.text.contains("Demo rule")
        }));

        // 2. Change (Up, persistent) + FlagForGround + DemoHeuristic
        let mut change_up = sample_eval(
            Status::Change,
            Escalation::FlagForGround,
            SourceKind::DemoHeuristic,
        );
        change_up.direction = Some(Direction::Up);
        change_up.persistent = true;
        change_up.source_ref = None;
        let f_change = render_facts(&change_up);
        assert!(
            f_change
                .iter()
                .any(|f| f.id == FactId::Status && f.text.contains("above your recent range"))
        );
        assert!(f_change.iter().any(|f| {
            f.id == FactId::Direction
                && f.text.contains("upward")
                && f.text.contains("persistent")
                && !f.text.contains("not persistent")
        }));
        assert!(
            f_change
                .iter()
                .any(|f| f.id == FactId::Escalation && f.text.contains("flagged for ground review"))
        );
        assert!(f_change.iter().any(|f| {
            f.id == FactId::Source
                && f.text.contains("demo heuristic")
                && f.text.contains("Demo rule, not medical advice")
        }));

        // 3. Change (Down, non-persistent) + SelfCare
        let mut change_down =
            sample_eval(Status::Change, Escalation::SelfCare, SourceKind::Published);
        change_down.direction = Some(Direction::Down);
        change_down.persistent = false;
        let f_down = render_facts(&change_down);
        assert!(
            f_down
                .iter()
                .any(|f| f.id == FactId::Status && f.text.contains("below your recent range"))
        );
        assert!(f_down.iter().any(|f| {
            f.id == FactId::Direction
                && f.text.contains("downward")
                && f.text.contains("not persistent")
        }));
        assert!(
            f_down
                .iter()
                .any(|f| f.id == FactId::Escalation && f.text.contains("self-care"))
        );

        // 4. Alarm + Urgent + ApprovedProtocol -> acción exacta de RF-10
        let alarm = sample_eval(
            Status::Alarm,
            Escalation::Urgent,
            SourceKind::ApprovedProtocol,
        );
        let f_alarm = render_facts(&alarm);
        assert!(
            f_alarm
                .iter()
                .any(|f| f.id == FactId::Status && f.text.contains("Alarm"))
        );
        assert!(f_alarm.iter().any(|f| {
            f.id == FactId::Escalation
                && f.text.contains(
                    "notify the onboard CMO, open the offline protocol and send to ground",
                )
        }));
        assert!(
            f_alarm
                .iter()
                .any(|f| f.id == FactId::Source && f.text.contains("approved protocol"))
        );

        // 5. Todos los motivos de NotEvaluated
        for (reason, expected_substr) in [
            (NotEvaluatedReason::NoData, "No data available"),
            (
                NotEvaluatedReason::TooFewData,
                "Need 3 previous readings (1/3 available)",
            ),
            (NotEvaluatedReason::Stale, "too old"),
            (NotEvaluatedReason::NotApplicable, "Context-only"),
        ] {
            let mut eval = sample_eval(
                Status::NotEvaluated,
                Escalation::Continue,
                SourceKind::DemoHeuristic,
            );
            eval.reason = Some(reason);
            eval.history_count = 1;
            eval.history_needed = 3;
            let facts = render_facts(&eval);
            assert!(
                facts
                    .iter()
                    .any(|f| f.id == FactId::Reason && f.text.contains(expected_substr)),
                "missing reason text for {reason:?}"
            );
            // Serialización JSON de FactLine y FactId
            let json = serde_json::to_string(&facts).unwrap();
            assert!(json.contains("\"id\":\"status\""));
        }

        // 6. Síntoma con zona corporal y descripción libre no filtra la descripción al vocabulario
        let mut symptom = sample_eval(
            Status::Change,
            Escalation::SelfCare,
            SourceKind::DemoHeuristic,
        );
        symptom.indicator = "symptom".into();
        symptom.body_zone = Some(BodyZone::LeftFoot);
        symptom.latest_value =
            Some(json!({ "body_zone": "left_foot", "score": 6, "description": "take aspirin" }));
        let f_symptom = render_facts(&symptom);
        assert!(
            f_symptom
                .iter()
                .any(|f| f.id == FactId::Indicator && f.text.contains("left foot"))
        );
        assert!(
            f_symptom
                .iter()
                .any(|f| f.id == FactId::LatestReading && f.text.contains("6/10"))
        );
        assert!(
            !f_symptom
                .iter()
                .any(|f| f.text.contains("aspirin") || f.text.contains("take"))
        );
    }

    // --- Motor con basura y errores de control ---

    #[test]
    fn garbage_engine_output_returns_ok_with_facts_and_note_none() {
        let eval = sample_eval(Status::Alarm, Escalation::Urgent, SourceKind::DemoHeuristic);
        let engine = MockEngine::with_responses(["asdf qwerty 999 !!! ¿hola?"])
            .with_model_id("space-health-v5");
        let mut explainer = SlmExplainer::with_engine(engine);
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));

        let exp = explainer.explain(&eval, &docs(), &mut ctl).unwrap();
        assert_eq!(exp.facts, render_facts(&eval));
        assert!(exp.note.is_none());
        assert!(exp.note_discarded.is_some());
        assert_eq!(exp.model_id, "space-health-v5");
    }

    #[test]
    fn explain_checks_cancel_and_deadline_after_progress_callbacks() {
        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let valid = "Your heart rate is stable within your recent range.";

        for stage in [Progress::Validating, Progress::Done] {
            let engine = MockEngine::with_responses([valid]);
            let mut explainer = SlmExplainer::new(engine, "space-health-v5");
            let cancel = CancelToken::new();
            let cancel_cb = cancel.clone();
            let mut ctl = Control::new(
                cancel,
                Deadline::after(Duration::from_secs(5)),
                move |progress| {
                    if progress == stage {
                        cancel_cb.cancel();
                    }
                },
            );
            assert_eq!(
                explainer.explain(&eval, &docs(), &mut ctl),
                Err(SlmError::Cancelled)
            );
        }

        let start = Instant::now();
        let expired = Arc::new(AtomicBool::new(false));
        let expired_clock = Arc::clone(&expired);
        let deadline = Deadline::with_clock(
            start,
            Duration::from_secs(5),
            Arc::new(move || {
                if expired_clock.load(Ordering::SeqCst) {
                    start + Duration::from_secs(10)
                } else {
                    start
                }
            }),
        );
        let expired_cb = Arc::clone(&expired);
        let mut ctl = Control::new(CancelToken::new(), deadline, move |progress| {
            if progress == Progress::Validating {
                expired_cb.store(true, Ordering::SeqCst);
            }
        });
        let engine = MockEngine::with_responses([valid]);
        let mut explainer = SlmExplainer::new(engine, "space-health-v5");
        assert_eq!(
            explainer.explain(&eval, &docs(), &mut ctl),
            Err(SlmError::Timeout)
        );
    }

    #[test]
    fn explainer_uses_deterministic_seed_and_temperature_and_supports_custom_prompt_format() {
        struct V5StubFormat;
        impl PromptFormat for V5StubFormat {
            fn system_prompt(&self) -> &str {
                "v5-system"
            }
            fn format_prompt(
                &self,
                eval: &Evaluation,
                _facts: &[FactLine],
                docs: &[Document],
            ) -> String {
                format!("source={} docs={}", eval.indicator, docs.len())
            }
        }

        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let engine =
            MockEngine::with_responses(["Your heart rate is stable within your recent range."]);
        let mut explainer = SlmExplainer::new(engine, "v5");
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));
        let _ = explainer.explain(&eval, &docs(), &mut ctl).unwrap();

        let req = explainer.engine().requests().first().unwrap();
        assert_eq!(req.temperature, DEFAULT_TEMPERATURE);
        assert_eq!(req.seed, DEFAULT_SEED);
        assert!(req.prompt.contains("Status: Stable"));
        assert!(req.prompt.contains("[hr-rest-1]"));

        let custom_engine =
            MockEngine::with_responses(["Your heart rate is stable within your recent range."]);
        let mut custom_explainer =
            SlmExplainer::new(custom_engine, "v5").with_prompt_format(V5StubFormat);
        let _ = custom_explainer.explain(&eval, &docs(), &mut ctl).unwrap();
        let custom_req = custom_explainer.engine().requests().first().unwrap();
        assert_eq!(custom_req.system, "v5-system");
        assert_eq!(custom_req.prompt, "source=heart_rate docs=2");
    }

    // --- Regresiones de validación: números Unicode, ordinales, negaciones, citas, longitud y fichas ---

    #[test]
    fn rejects_unicode_digits_ordinals_multipliers_and_spanish_numbers() {
        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let facts = render_facts(&eval);

        for bad in [
            "Your heart rate is ٧٢.",
            "Your heart rate is ⁷².",
            "Your heart rate is Ⅱ.",
            "Your first reading is stable.",
            "La segunda lectura es estable.",
            "Your heart rate is half of baseline.",
            "Your heart rate is twice your baseline.",
            "Your heart rate has a dozen readings.",
            "Su pulso es cien.",
            "Un pulso estable.",
        ] {
            let res = validate_note(bad, &facts, &docs());
            assert!(res.is_err(), "should reject numeric/ordinal note: {bad}");
        }
    }

    #[test]
    fn negation_requires_exact_literal_sentence_from_facts_or_cited_docs() {
        let alarm = sample_eval(Status::Alarm, Escalation::Urgent, SourceKind::DemoHeuristic);
        let facts = render_facts(&alarm);
        let docs_with_neg = vec![Document {
            id: "cmo-alarm".into(),
            text: "Do not delay contacting the onboard CMO. Open the offline protocol.".into(),
        }];

        // Negación que invierte la acción -> rechazada
        assert_eq!(
            validate_note(
                "Do not contact the onboard CMO [cmo-alarm].",
                &facts,
                &docs_with_neg
            ),
            Err(ERR_FORBIDDEN_NEGATION)
        );
        assert_eq!(
            validate_note(
                "Do not open the offline protocol [cmo-alarm].",
                &facts,
                &docs_with_neg
            ),
            Err(ERR_FORBIDDEN_NEGATION)
        );

        // Frase literal negada de los hechos ("Demo rule, not medical advice.") -> permitida
        assert!(validate_note("Demo rule, not medical advice.", &facts, &docs_with_neg).is_ok());

        // Frase literal negada de una ficha citada -> permitida si se cita esa ficha
        assert!(
            validate_note(
                "Do not delay contacting the onboard CMO [cmo-alarm].",
                &facts,
                &docs_with_neg
            )
            .is_ok()
        );

        // Sin citar la ficha, esa misma frase negada es rechazada
        assert_eq!(
            validate_note(
                "Do not delay contacting the onboard CMO.",
                &facts,
                &docs_with_neg
            ),
            Err(ERR_FORBIDDEN_NEGATION)
        );
    }

    #[test]
    fn enforces_citations_length_sentences_and_defense_in_depth() {
        let alarm = sample_eval(Status::Alarm, Escalation::Urgent, SourceKind::Published);
        let f_alarm = render_facts(&alarm);

        // Corchetes mal formados o desconocidos
        for bad_cite in [
            "Your heart rate is in alarm [cmo-alarm.",
            "Your heart rate is in alarm [].",
            "Your heart rate is in alarm [   ].",
            "Your heart rate is in alarm cmo-alarm].",
        ] {
            assert_eq!(
                validate_note(bad_cite, &f_alarm, &docs()),
                Err(ERR_MALFORMED_CITATION)
            );
        }
        assert_eq!(
            validate_note(
                "Your heart rate is in alarm [missing-doc].",
                &f_alarm,
                &docs()
            ),
            Err(ERR_UNKNOWN_CITATION)
        );

        // Defensa en profundidad incluso si una ficha contiene una palabra incompatible
        let misleading_doc = vec![Document {
            id: "bad-advice".into(),
            text: "You can wait and postpone action because you are safe.".into(),
        }];
        assert_eq!(
            validate_note(
                "You can wait because you are safe [bad-advice].",
                &f_alarm,
                &misleading_doc
            ),
            Err(ERR_INCOMPATIBLE_STATUS)
        );

        // Palabras de una ficha NO citada no forman parte del vocabulario permitido
        let extra_doc = vec![Document {
            id: "sleep-doc".into(),
            text: "Hydration and quiet rest help recovery.".into(),
        }];
        let stable = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let f_stable = render_facts(&stable);
        assert_eq!(
            validate_note("Hydration and quiet rest help.", &f_stable, &extra_doc),
            Err(ERR_OUT_OF_VOCABULARY)
        );

        // Límites de longitud y frases
        let four_sentences = "Heart rate is stable. Your reading is within range. History is available. Continue routine monitoring.";
        assert_eq!(
            validate_note(four_sentences, &f_stable, &docs()),
            Err(ERR_SENTENCE_COUNT)
        );

        let too_many_words = "Your heart rate is stable within your recent range and your heart rate is stable within your recent range and your heart rate is stable within your recent range and stable.";
        assert_eq!(
            validate_note(too_many_words, &f_stable, &docs()),
            Err(ERR_SENTENCE_TOO_LONG)
        );

        let over_350 = "Your heart rate is stable within your recent range. ".repeat(8);
        assert_eq!(
            validate_note(&over_350, &f_stable, &docs()),
            Err(ERR_TOO_LONG)
        );
    }

    #[test]
    fn engine_errors_propagate_without_turning_into_ok() {
        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));

        for err in [
            SlmError::Engine("backend failure".into()),
            SlmError::Cancelled,
            SlmError::Timeout,
        ] {
            let mut engine = MockEngine::new();
            engine.push_error(err.clone());
            let mut explainer = SlmExplainer::with_engine(engine);
            assert_eq!(explainer.explain(&eval, &docs(), &mut ctl), Err(err));
        }
    }

    #[test]
    fn render_facts_formats_indicator_units_and_event_alarms() {
        let mut bp = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        bp.indicator = "blood_pressure".into();
        bp.latest_value = Some(json!({ "systolic": 118, "diastolic": 76 }));
        bp.baseline = Some(120.0);
        let f_bp = render_facts(&bp);
        assert!(
            f_bp.iter()
                .any(|f| { f.id == FactId::LatestReading && f.text.contains("118/76 mmHg") })
        );
        assert!(
            f_bp.iter()
                .any(|f| f.id == FactId::Baseline && f.text.contains("120 mmHg"))
        );

        let mut spo2 = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        spo2.indicator = "oxygen_in_blood".into();
        spo2.latest_value = Some(json!({ "percentage": 98.2 }));
        spo2.baseline = Some(98.0);
        let f_spo2 = render_facts(&spo2);
        assert!(
            f_spo2
                .iter()
                .any(|f| f.id == FactId::LatestReading && f.text.contains("98.2%"))
        );

        let mut sleep = sample_eval(
            Status::Change,
            Escalation::SelfCare,
            SourceKind::DemoHeuristic,
        );
        sleep.indicator = "sleep".into();
        sleep.latest_value = Some(json!({ "hours": 5.5, "quality": "poor" }));
        let f_sleep = render_facts(&sleep);
        assert!(f_sleep.iter().any(|f| {
            f.id == FactId::LatestReading && f.text.contains("5.5 h (quality: poor)")
        }));

        let mut event_alarm = sample_eval(
            Status::Alarm,
            Escalation::Urgent,
            SourceKind::ApprovedProtocol,
        );
        event_alarm.indicator = "need_help".into();
        event_alarm.latest_value = None;
        event_alarm.latest_measured_at = None;
        event_alarm.baseline = None;
        let f_event = render_facts(&event_alarm);
        assert!(
            f_event
                .iter()
                .any(|f| f.id == FactId::Indicator && f.text.contains("Need help event"))
        );
        assert!(
            f_event
                .iter()
                .any(|f| f.id == FactId::LatestReading && f.text.contains("none"))
        );
    }
}
