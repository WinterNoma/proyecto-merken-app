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
        error::{EngineErrorCode, SlmError},
        explain::{
            AI_SUGGESTION_LABEL, ERR_DUPLICATE_SELECTION, ERR_EXTRA_FIELDS, ERR_INVALID_JSON,
            ERR_INVALID_SCHEMA, ERR_OUTPUT_TOO_LARGE, ERR_TOO_MANY_SELECTIONS,
            ERR_UNKNOWN_SELECTION, Explainer, FactId, FactLine, Ficha, FichaId, MAX_ID_LEN,
            MAX_SELECTION_OUTPUT_BYTES, MAX_STATEMENT_CHARS, OfferedStatement, PromptFormat,
            SlmExplainer, Statement, StatementId, build_selection_grammar, offered_statements,
            render_facts, validate_selection,
        },
    };
    use pulsys_core::{
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

    fn stmt(id: &str, text: &str, applies_to: Vec<Status>) -> Statement {
        Statement::new(StatementId::new(id).unwrap(), text, applies_to).unwrap()
    }

    fn ficha(id: &str, statements: Vec<Statement>) -> Ficha {
        Ficha::new(FichaId::new(id).unwrap(), statements).unwrap()
    }

    fn sample_fichas() -> Vec<Ficha> {
        vec![
            ficha(
                "hr-rest-1",
                vec![
                    stmt(
                        "compare-baseline",
                        "Compare resting pulse against your personal baseline after 5 minutes of rest.",
                        vec![Status::Stable, Status::Change],
                    ),
                    stmt(
                        "log-more",
                        "Log additional baseline readings so trend evaluation can run.",
                        vec![Status::NotEvaluated],
                    ),
                ],
            ),
            ficha(
                "safety",
                vec![stmt(
                    "no-aspirin",
                    "Do not take aspirin.",
                    vec![Status::Stable, Status::Change, Status::Alarm],
                )],
            ),
            ficha(
                "workload",
                vec![stmt(
                    "no-double",
                    "Do not double workload during rest.",
                    vec![Status::Stable, Status::Change],
                )],
            ),
            ficha(
                "lighting",
                vec![stmt(
                    "lower-cabin",
                    "Lower cabin lighting during rest.",
                    vec![Status::Stable, Status::Change],
                )],
            ),
            ficha(
                "cmo-alarm",
                vec![stmt(
                    "notify-cmo",
                    "Notify the onboard CMO and open the offline protocol.",
                    vec![Status::Alarm],
                )],
            ),
        ]
    }

    fn explain_with_output(
        eval: &Evaluation,
        fichas: &[Ficha],
        raw_output: &str,
    ) -> crate::explain::Explanation {
        let engine = MockEngine::with_responses([raw_output]).with_model_id("space-health-v5");
        let mut explainer = SlmExplainer::with_engine(engine);
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));
        explainer.explain(eval, fichas, &mut ctl).unwrap()
    }

    // --- Parser mínimo del subconjunto GBNF emitido por `build_selection_grammar` para verificarla ---

    fn parse_quoted_gbnf_terminal(rest: &str) -> Option<(String, &str)> {
        let after_open = rest.strip_prefix('"')?;
        let mut out = String::new();
        let mut chars = after_open.char_indices();
        while let Some((idx, ch)) = chars.next() {
            match ch {
                '"' => return Some((out, &after_open[idx + 1..])),
                '\\' => {
                    let (_, esc) = chars.next()?;
                    match esc {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'x' => {
                            let (_, h1) = chars.next()?;
                            let (_, h2) = chars.next()?;
                            let hex: String = [h1, h2].into_iter().collect();
                            let byte = u8::from_str_radix(&hex, 16).ok()?;
                            out.push(byte as char);
                        }
                        other => out.push(other),
                    }
                }
                other => out.push(other),
            }
        }
        None
    }

    fn extract_gbnf_id_literals(grammar: &str) -> Option<Vec<String>> {
        let id_line = grammar.lines().find(|l| l.starts_with("id ::="))?;
        let mut rest = id_line.strip_prefix("id ::=")?.trim();
        let mut ids = Vec::new();
        loop {
            let (tok, next) = parse_quoted_gbnf_terminal(rest)?;
            let parsed_id: String = serde_json::from_str(&tok).ok()?;
            ids.push(parsed_id);
            let after_ws = next.trim_start();
            if after_ws.is_empty() {
                break;
            }
            rest = after_ws.strip_prefix('|')?.trim_start();
        }
        Some(ids)
    }

    fn grammar_accepts(grammar: &str, candidate: &str) -> bool {
        let s = candidate.trim();
        let Some(after_brace) = s.strip_prefix('{') else {
            return false;
        };
        let Some(after_key) = after_brace.trim_start().strip_prefix("\"select\"") else {
            return false;
        };
        let Some(after_colon) = after_key.trim_start().strip_prefix(':') else {
            return false;
        };
        let Some(after_lbracket) = after_colon.trim_start().strip_prefix('[') else {
            return false;
        };
        let Some(inside_and_rest) = after_lbracket.trim_start().strip_suffix('}') else {
            return false;
        };
        let Some(inside) = inside_and_rest.trim_end().strip_suffix(']') else {
            return false;
        };
        let inside = inside.trim();
        if inside.is_empty() {
            return grammar.contains("\"[\" ws \"]\"");
        }
        let Some(allowed_ids) = extract_gbnf_id_literals(grammar) else {
            return false;
        };
        let Ok(items) = serde_json::from_str::<Vec<String>>(&format!("[{inside}]")) else {
            return false;
        };
        !items.is_empty() && items.len() <= 3 && items.iter().all(|id| allowed_ids.contains(id))
    }

    // --- Pruebas obligatorias contra todos los ataques de la revisión ---

    #[test]
    fn revision_attacks_are_impossible_or_discarded_under_authorized_selection() {
        let stable = sample_eval(
            Status::Stable,
            Escalation::Continue,
            SourceKind::DemoHeuristic,
        );
        let alarm = sample_eval(Status::Alarm, Escalation::Urgent, SourceKind::DemoHeuristic);
        let fichas = sample_fichas();

        // 1. "You have a clot" / prosa libre -> descartado (no es JSON válido ni ID)
        let exp = explain_with_output(&stable, &fichas, "You have a clot.");
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_INVALID_JSON));
        assert!(!exp.facts.is_empty());

        // 2. "Take aspirin [safety]" a partir de "Do not take aspirin." -> imposible;
        // texto libre se descarta y seleccionar la ficha siempre renderiza el texto curado literal.
        let exp_prose = explain_with_output(&stable, &fichas, "Take aspirin [safety].");
        assert!(exp_prose.note.is_none());
        assert_eq!(exp_prose.note_discarded, Some(ERR_INVALID_JSON));

        let exp_selected =
            explain_with_output(&stable, &fichas, r#"{"select":["safety/no-aspirin"]}"#);
        let note = exp_selected.note.unwrap();
        assert_eq!(note.label, AI_SUGGESTION_LABEL);
        assert_eq!(note.statements.len(), 1);
        assert_eq!(note.statements[0].text, "Do not take aspirin.");
        assert!(exp_selected.note_discarded.is_none());

        // 3. "doubled", "percents", "lowered" -> descartados porque solo se aceptan IDs JSON
        for attack in [
            "Your heart rate has doubled [workload].",
            "Your pulse fell by percents [hr-rest-1].",
            "Your heart rate is lowered from your personal baseline [lighting].",
            "The recommended action is \u{202E}on\u{202C} monitoring.",
            "Continue routine monitoring [take two aspirin].",
            "You are safe; contacting the CMO is unnecessary.",
            "Your health is excellent.",
        ] {
            let exp = explain_with_output(&alarm, &fichas, attack);
            assert!(exp.note.is_none(), "attack must be discarded: {attack}");
            assert!(exp.note_discarded.is_some());
        }

        // 4. Intento de inyectar texto o bidi dentro de un ID devuelto por el modelo
        for bad_id in [
            r#"{"select":["safety/\u202Eno-aspirin"]}"#,
            r#"{"select":["take two aspirin"]}"#,
            r#"{"select":["Take aspirin [safety]"]}"#,
        ] {
            let exp = explain_with_output(&stable, &fichas, bad_id);
            assert!(exp.note.is_none());
            assert_eq!(exp.note_discarded, Some(ERR_UNKNOWN_SELECTION));
        }
    }

    // --- Validación de IDs opacos y textos curados al cargar ---

    #[test]
    fn closed_ids_and_curated_text_reject_invalid_and_bidi_inputs_on_load() {
        // IDs válidos en los bordes [a-z0-9-]{1,40}
        assert!(FichaId::new("a").is_ok());
        assert!(FichaId::new("a-09-z").is_ok());
        assert!(FichaId::new("a".repeat(MAX_ID_LEN)).is_ok());

        // IDs inválidos
        for bad in [
            "",
            &"a".repeat(MAX_ID_LEN + 1),
            "Take-Aspirin",
            "take two aspirin",
            "hr_rest",
            "ficha/stmt",
            "[safety]",
            "ficha\u{202E}1",
            "café",
        ] {
            assert!(FichaId::new(bad).is_err(), "should reject FichaId: {bad:?}");
            assert!(
                StatementId::new(bad).is_err(),
                "should reject StatementId: {bad:?}"
            );
        }

        // Frase curada con bidi, caracteres de formato, controles y texto no inglés rechazada al cargar
        let valid_id = StatementId::new("s-1").unwrap();
        for bad_text in [
            "Check pulse \u{202E}on\u{202C} rest.",
            "Check pulse \u{2066}hidden\u{2069}.",
            "Check\u{200B} pulse.",
            "\u{FEFF}Check pulse.",
            "Check\u{00AD}pulse.",
            "Line 1\nLine 2",
            "Tab\there",
            "Null\0byte",
            "Tu pulso está estable.",
            "¿Cómo estás?",
            "Your heart rate is st\u{0430}ble.",
            "   ",
            "...",
            &"a".repeat(MAX_STATEMENT_CHARS + 1),
        ] {
            assert!(
                Statement::new(valid_id.clone(), bad_text, vec![Status::Stable]).is_err(),
                "should reject curated text: {bad_text:?}"
            );
        }

        // Carga JSON con bidi o ID con texto también se rechaza sin filtrar datos
        let bidi_json = json!({
            "id": "safety",
            "statements": [{
                "id": "s-1",
                "text": "The action is \u{202E}on\u{202C} monitoring.",
                "applies_to": ["stable"]
            }]
        })
        .to_string();
        assert!(Ficha::from_json(&bidi_json).is_err());

        let bad_id_json = json!({
            "id": "take two aspirin",
            "statements": []
        })
        .to_string();
        let err = Ficha::from_json(&bad_id_json).unwrap_err();
        assert!(!err.to_string().contains("aspirin"));

        // Duplicados de statement dentro de una ficha y de ficha en vec_from_json
        let dup_stmt = Ficha::new(
            FichaId::new("dup").unwrap(),
            vec![
                stmt("same", "Rest for five minutes.", vec![Status::Stable]),
                stmt("same", "Drink water.", vec![Status::Stable]),
            ],
        );
        assert!(dup_stmt.is_err());

        let dup_fichas = json!([
            { "id": "f-1", "statements": [] },
            { "id": "f-1", "statements": [] }
        ])
        .to_string();
        assert!(Ficha::vec_from_json(&dup_fichas).is_err());
    }

    // --- Filtro determinista previo, selección 0..3 y todos los casos de rechazo ---

    #[test]
    fn status_filter_and_selection_validation_cover_all_error_and_empty_cases() {
        let fichas = sample_fichas();
        let stable_eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let alarm_eval = sample_eval(
            Status::Alarm,
            Escalation::Urgent,
            SourceKind::ApprovedProtocol,
        );

        let stable_offered = offered_statements(&fichas, stable_eval.status).unwrap();
        let stable_ids: Vec<&str> = stable_offered
            .iter()
            .map(|o| o.selection_id.as_str())
            .collect();
        assert!(stable_ids.contains(&"hr-rest-1/compare-baseline"));
        assert!(stable_ids.contains(&"safety/no-aspirin"));
        assert!(!stable_ids.contains(&"cmo-alarm/notify-cmo"));
        assert!(!stable_ids.contains(&"hr-rest-1/log-more"));

        let alarm_offered = offered_statements(&fichas, alarm_eval.status).unwrap();
        let alarm_ids: Vec<&str> = alarm_offered
            .iter()
            .map(|o| o.selection_id.as_str())
            .collect();
        assert!(alarm_ids.contains(&"cmo-alarm/notify-cmo"));
        assert!(alarm_ids.contains(&"safety/no-aspirin"));
        assert!(!alarm_ids.contains(&"hr-rest-1/compare-baseline"));

        // 0 seleccionadas -> note: None sin descarte
        let exp_empty = explain_with_output(&stable_eval, &fichas, r#"{"select":[]}"#);
        assert!(exp_empty.note.is_none());
        assert!(exp_empty.note_discarded.is_none());

        // Orden preservado y texto exacto de la ficha (hasta 3)
        let exp_three = explain_with_output(
            &stable_eval,
            &fichas,
            r#"{"select":["lighting/lower-cabin","safety/no-aspirin","hr-rest-1/compare-baseline"]}"#,
        );
        assert!(exp_three.note_discarded.is_none());
        let note = exp_three.note.unwrap();
        assert_eq!(note.label, AI_SUGGESTION_LABEL);
        assert_eq!(note.statements.len(), 3);
        assert_eq!(note.statements[0].ficha_id.as_str(), "lighting");
        assert_eq!(note.statements[0].statement_id.as_str(), "lower-cabin");
        assert_eq!(note.statements[0].text, "Lower cabin lighting during rest.");
        assert_eq!(note.statements[1].text, "Do not take aspirin.");
        assert_eq!(
            note.statements[2].text,
            "Compare resting pulse against your personal baseline after 5 minutes of rest."
        );

        // ID inexistente
        assert_eq!(
            validate_selection(r#"{"select":["does-not/exist"]}"#, &stable_offered),
            Err(ERR_UNKNOWN_SELECTION)
        );

        // ID no ofrecido (de otro estado: cmo-alarm/notify-cmo en Stable)
        let exp_wrong_status = explain_with_output(
            &stable_eval,
            &fichas,
            r#"{"select":["cmo-alarm/notify-cmo"]}"#,
        );
        assert!(exp_wrong_status.note.is_none());
        assert_eq!(exp_wrong_status.note_discarded, Some(ERR_UNKNOWN_SELECTION));

        // Duplicado
        let exp_dup = explain_with_output(
            &stable_eval,
            &fichas,
            r#"{"select":["safety/no-aspirin","safety/no-aspirin"]}"#,
        );
        assert!(exp_dup.note.is_none());
        assert_eq!(exp_dup.note_discarded, Some(ERR_DUPLICATE_SELECTION));

        // Más de 3
        let exp_four = explain_with_output(
            &stable_eval,
            &fichas,
            r#"{"select":["hr-rest-1/compare-baseline","safety/no-aspirin","workload/no-double","lighting/lower-cabin"]}"#,
        );
        assert!(exp_four.note.is_none());
        assert_eq!(exp_four.note_discarded, Some(ERR_TOO_MANY_SELECTIONS));

        // JSON con campos extra
        let exp_extra = explain_with_output(
            &stable_eval,
            &fichas,
            r#"{"select":["safety/no-aspirin"],"extra":"Take aspirin"}"#,
        );
        assert!(exp_extra.note.is_none());
        assert_eq!(exp_extra.note_discarded, Some(ERR_EXTRA_FIELDS));

        // JSON roto
        let exp_broken =
            explain_with_output(&stable_eval, &fichas, r#"{"select":["safety/no-aspirin""#);
        assert!(exp_broken.note.is_none());
        assert_eq!(exp_broken.note_discarded, Some(ERR_INVALID_JSON));

        // Esquema inválido (objeto sin select, array raíz, tipo erróneo)
        for bad_schema in [
            r#"{}"#,
            r#"["safety/no-aspirin"]"#,
            r#"{"select":"safety/no-aspirin"}"#,
            r#"{"select":[123]}"#,
        ] {
            assert_eq!(
                validate_selection(bad_schema, &stable_offered),
                Err(ERR_INVALID_SCHEMA)
            );
        }

        // Salida que excede límite de bytes
        let huge = format!(
            "{{\"select\":[]}}{}",
            " ".repeat(MAX_SELECTION_OUTPUT_BYTES)
        );
        assert_eq!(
            validate_selection(&huge, &stable_offered),
            Err(ERR_OUTPUT_TOO_LARGE)
        );
    }

    // --- Gramática GBNF generada dinámicamente ---

    #[test]
    fn generated_selection_grammar_allows_only_offered_ids_and_escapes_safely() {
        // Sin IDs ofrecidos: solo {"select": []}
        let empty_grammar = build_selection_grammar(&[]);
        assert!(grammar_accepts(&empty_grammar, r#"{"select": []}"#));
        assert!(!grammar_accepts(
            &empty_grammar,
            r#"{"select": ["safety/no-aspirin"]}"#
        ));

        // Con IDs ofrecidos: admite 0..3 ofrecidos y rechaza no ofrecidos, 4+ o prosa
        let grammar = build_selection_grammar(&["hr-rest-1/compare-baseline", "safety/no-aspirin"]);
        assert!(grammar_accepts(&grammar, r#"{"select":[]}"#));
        assert!(grammar_accepts(
            &grammar,
            r#"{"select":["safety/no-aspirin"]}"#
        ));
        assert!(grammar_accepts(
            &grammar,
            r#"{"select":["hr-rest-1/compare-baseline","safety/no-aspirin"]}"#
        ));
        assert!(grammar_accepts(
            &grammar,
            r#"{"select":["safety/no-aspirin","hr-rest-1/compare-baseline","safety/no-aspirin"]}"#
        ));
        assert!(!grammar_accepts(
            &grammar,
            r#"{"select":["cmo-alarm/notify-cmo"]}"#
        ));
        assert!(!grammar_accepts(
            &grammar,
            r#"{"select":["safety/no-aspirin","safety/no-aspirin","safety/no-aspirin","safety/no-aspirin"]}"#
        ));
        assert!(!grammar_accepts(&grammar, "You have a clot."));

        // IDs fuera del conjunto permitido se rechazan.
        assert!(build_selection_grammar(&[r#"a"b\c"#]).is_empty());

        // `SlmExplainer` envía la gramática generada por petición excluyendo otros estados
        let stable = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let fichas = sample_fichas();
        let engine = MockEngine::with_responses([r#"{"select":["hr-rest-1/compare-baseline"]}"#]);
        let mut explainer = SlmExplainer::new(engine, "v5");
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));
        let exp = explainer.explain(&stable, &fichas, &mut ctl).unwrap();
        assert!(exp.note.is_some());

        let req = explainer.engine().requests().first().unwrap();
        let sent_grammar = req.grammar.as_deref().unwrap();
        assert!(sent_grammar.contains("hr-rest-1/compare-baseline"));
        assert!(sent_grammar.contains("safety/no-aspirin"));
        assert!(!sent_grammar.contains("cmo-alarm/notify-cmo"));
        assert!(!req.prompt.contains("cmo-alarm/notify-cmo"));
        assert!(req.prompt.contains("[hr-rest-1/compare-baseline]"));
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
        assert!(f_stable.iter().any(|f| {
            f.id == FactId::Escalation && f.text.contains("continue routine monitoring")
        }));
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
            let json = serde_json::to_string(&facts).unwrap();
            assert!(json.contains("\"id\":\"status\""));
        }

        // 6. Síntoma con zona corporal y descripción libre no filtra la descripción
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

    // --- Control, PromptFormat y propagación de errores del motor ---

    #[test]
    fn garbage_engine_output_returns_ok_with_facts_and_note_none() {
        let eval = sample_eval(Status::Alarm, Escalation::Urgent, SourceKind::DemoHeuristic);
        let engine = MockEngine::with_responses(["asdf qwerty 999 !!! ¿hola?"])
            .with_model_id("space-health-v5");
        let mut explainer = SlmExplainer::with_engine(engine);
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));

        let exp = explainer
            .explain(&eval, &sample_fichas(), &mut ctl)
            .unwrap();
        assert_eq!(exp.facts, render_facts(&eval));
        assert!(exp.note.is_none());
        assert_eq!(exp.note_discarded, Some(ERR_INVALID_JSON));
        assert_eq!(exp.model_id, "space-health-v5");
    }

    #[test]
    fn explain_checks_cancel_and_deadline_after_progress_callbacks() {
        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let valid = r#"{"select":["hr-rest-1/compare-baseline"]}"#;
        let fichas = sample_fichas();

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
                explainer.explain(&eval, &fichas, &mut ctl),
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
            explainer.explain(&eval, &fichas, &mut ctl),
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
                offered: &[OfferedStatement],
            ) -> String {
                format!("source={} offered={}", eval.indicator, offered.len())
            }
        }

        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let fichas = sample_fichas();
        let engine = MockEngine::with_responses([r#"{"select":[]}"#]);
        let mut explainer = SlmExplainer::new(engine, "v5");
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));
        let _ = explainer.explain(&eval, &fichas, &mut ctl).unwrap();

        let req = explainer.engine().requests().first().unwrap();
        assert_eq!(req.temperature, DEFAULT_TEMPERATURE);
        assert_eq!(req.seed, DEFAULT_SEED);
        assert!(req.prompt.contains("Status: Stable"));
        assert!(req.prompt.contains("[hr-rest-1/compare-baseline]"));

        let custom_engine = MockEngine::with_responses([r#"{"select":[]}"#]);
        let mut custom_explainer =
            SlmExplainer::new(custom_engine, "v5").with_prompt_format(V5StubFormat);
        let _ = custom_explainer.explain(&eval, &fichas, &mut ctl).unwrap();
        let custom_req = custom_explainer.engine().requests().first().unwrap();
        assert_eq!(custom_req.system, "v5-system");
        assert_eq!(custom_req.prompt, "source=heart_rate offered=4");
    }

    #[test]
    fn engine_errors_propagate_without_turning_into_ok_and_use_constant_codes() {
        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let fichas = sample_fichas();
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));

        for err in [
            SlmError::Engine(EngineErrorCode::GenerationFailed),
            SlmError::Cancelled,
            SlmError::Timeout,
        ] {
            let mut engine = MockEngine::new();
            engine.push_error(err.clone());
            let mut explainer = SlmExplainer::with_engine(engine);
            assert_eq!(explainer.explain(&eval, &fichas, &mut ctl), Err(err));
        }

        assert_eq!(
            SlmError::Engine(EngineErrorCode::GenerationFailed).to_string(),
            "SLM engine error: generation_failed"
        );
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
