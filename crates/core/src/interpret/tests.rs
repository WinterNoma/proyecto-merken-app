//! Pruebas unitarias, regresiones de la revisión de Sol y propiedades del intérprete.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::*;
use crate::{
    catalog::{BodyZone, Indicator, SleepQuality},
    proposal::{InterpreterKind, PROPOSAL_SCHEMA_VERSION, ProposedRecord, TimeHint},
};
use proptest::prelude::*;
use serde_json::{Value, json};

fn field_val<'a>(rec: &'a ProposedRecord, key: &str) -> Option<&'a Value> {
    rec.fields
        .iter()
        .find(|f| f.key == key)
        .and_then(|f| f.value.as_ref())
}

fn field_quote<'a>(rec: &'a ProposedRecord, key: &str) -> Option<&'a str> {
    rec.fields
        .iter()
        .find(|f| f.key == key)
        .and_then(|f| f.quote.as_deref())
}

fn assert_valid(text: &str) -> Proposal {
    let p = interpret(text);
    let issues = p.check(text);
    assert!(issues.is_empty(), "check failed for {text:?}: {issues:?}");
    assert_eq!(p.schema_version, PROPOSAL_SCHEMA_VERSION);
    assert_eq!(p.interpreter, InterpreterKind::Rules);
    assert_eq!(p.interpreter_version, RULES_INTERPRETER_VERSION);
    p
}

#[test]
fn heart_rate_cases() {
    let p = assert_valid("pulso 72");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::HeartRate);
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));

    let p = assert_valid("pulso setenta y dos");
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));
    assert_eq!(field_quote(&p.records[0], "bpm"), Some("setenta y dos"));

    let p = assert_valid("Frecuencia cardíaca ciento setenta y dos");
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(172)));
    assert_eq!(
        field_quote(&p.records[0], "bpm"),
        Some("ciento setenta y dos")
    );

    let p = assert_valid("My heart rate is seventy two bpm");
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));

    let p = assert_valid("HR one hundred seventy two");
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(172)));

    let p = assert_valid("84 lpm");
    assert_eq!(p.records[0].indicator, Indicator::HeartRate);
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(84)));
}

#[test]
fn six_vs_sixteen_and_seventy_two_vs_one_hundred_seventy_two() {
    let p = assert_valid("me duele la cabeza un seis");
    assert_eq!(field_val(&p.records[0], "score"), Some(&json!(6)));

    let p = assert_valid("me duele la cabeza dieciséis");
    assert_eq!(p.records[0].indicator, Indicator::Symptom);
    assert_eq!(field_val(&p.records[0], "score"), None);

    let p = assert_valid("pulso dieciséis");
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(16)));

    let p = assert_valid("my left foot hurts six");
    assert_eq!(field_val(&p.records[0], "score"), Some(&json!(6)));

    let p = assert_valid("my left foot hurts sixteen");
    assert_eq!(field_val(&p.records[0], "score"), None);

    let p = assert_valid("mood six");
    assert_eq!(field_val(&p.records[0], "score"), Some(&json!(6)));

    let p = assert_valid("pulse sixteen");
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(16)));
}

#[test]
fn blood_pressure_cases() {
    let p = assert_valid("presión 120/80");
    assert_eq!(p.records[0].indicator, Indicator::BloodPressure);
    assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(120)));
    assert_eq!(field_val(&p.records[0], "diastolic"), Some(&json!(80)));

    let p = assert_valid("tensión ciento veinte sobre ochenta");
    assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(120)));
    assert_eq!(field_val(&p.records[0], "diastolic"), Some(&json!(80)));

    let p = assert_valid("blood pressure 118 over 76 mmHg");
    assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(118)));
    assert_eq!(field_val(&p.records[0], "diastolic"), Some(&json!(76)));

    let p = assert_valid("120/80");
    assert_eq!(p.records[0].indicator, Indicator::BloodPressure);
    assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(120)));
    assert_eq!(field_val(&p.records[0], "diastolic"), Some(&json!(80)));

    let p = assert_valid("presión 130");
    assert_eq!(p.records[0].indicator, Indicator::BloodPressure);
    assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(130)));
    assert_eq!(field_val(&p.records[0], "diastolic"), None);
}

#[test]
fn temperature_glucose_oxygen_mood_cases() {
    let p = assert_valid("temperatura treinta y seis coma ocho");
    assert_eq!(p.records[0].indicator, Indicator::BodyTemperature);
    assert_eq!(field_val(&p.records[0], "celsius"), Some(&json!(36.8)));

    let p = assert_valid("36,8 °C");
    assert_eq!(p.records[0].indicator, Indicator::BodyTemperature);
    assert_eq!(field_val(&p.records[0], "celsius"), Some(&json!(36.8)));

    let p = assert_valid("Body temperature 37.4 celsius");
    assert_eq!(p.records[0].indicator, Indicator::BodyTemperature);
    assert_eq!(field_val(&p.records[0], "celsius"), Some(&json!(37.4)));

    let p = assert_valid("glucosa 95 mg/dL");
    assert_eq!(p.records[0].indicator, Indicator::Glucose);
    assert_eq!(field_val(&p.records[0], "mg_dl"), Some(&json!(95)));

    let p = assert_valid("blood sugar one hundred ten");
    assert_eq!(p.records[0].indicator, Indicator::Glucose);
    assert_eq!(field_val(&p.records[0], "mg_dl"), Some(&json!(110)));

    let p = assert_valid("SpO₂ 98.2%");
    assert_eq!(p.records[0].indicator, Indicator::OxygenInBlood);
    assert_eq!(field_val(&p.records[0], "percentage"), Some(&json!(98.2)));

    let p = assert_valid("saturación de oxígeno noventa y nueve");
    assert_eq!(p.records[0].indicator, Indicator::OxygenInBlood);
    assert_eq!(field_val(&p.records[0], "percentage"), Some(&json!(99)));

    let p = assert_valid("ánimo 6");
    assert_eq!(p.records[0].indicator, Indicator::Mood);
    assert_eq!(field_val(&p.records[0], "score"), Some(&json!(6)));

    let p = assert_valid("mood 8/10");
    assert_eq!(p.records[0].indicator, Indicator::Mood);
    assert_eq!(field_val(&p.records[0], "score"), Some(&json!(8)));
}

#[test]
fn sleep_cases() {
    let p = assert_valid("dormí cinco horas y media");
    let rec = &p.records[0];
    assert_eq!(rec.indicator, Indicator::Sleep);
    assert_eq!(field_val(rec, "hours"), Some(&json!(5.5)));
    assert_eq!(field_val(rec, "quality"), None);
    assert_eq!(field_val(rec, "window_start"), None);
    assert_eq!(field_val(rec, "window_end"), None);
    assert!(!rec.is_complete());

    let p = assert_valid("slept 6h");
    assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(6)));
    assert_eq!(field_val(&p.records[0], "window_start"), None);
    assert_eq!(field_val(&p.records[0], "window_end"), None);

    let p = assert_valid("dormí mal");
    assert_eq!(field_val(&p.records[0], "hours"), None);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));
    assert_eq!(field_val(&p.records[0], "window_start"), None);

    let p = assert_valid("slept badly");
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));

    let p = assert_valid("no dormí bien");
    assert!(p.negated.is_empty());
    assert_eq!(p.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));

    let p = assert_valid("sueño regular 7 horas");
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("fair")));
    assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(7)));

    let p = assert_valid("slept well for 8 hours");
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("good")));
    assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(8)));
}

#[test]
fn symptom_and_time_hint_cases() {
    let p = assert_valid("me duele la cabeza un 6");
    let rec = &p.records[0];
    assert_eq!(rec.indicator, Indicator::Symptom);
    assert_eq!(field_val(rec, "body_zone"), Some(&json!("head")));
    assert_eq!(field_val(rec, "score"), Some(&json!(6)));
    assert_eq!(
        field_val(rec, "description"),
        Some(&json!("me duele la cabeza un 6"))
    );

    let p = assert_valid("dolor de espalda 4/10");
    assert_eq!(field_val(&p.records[0], "body_zone"), Some(&json!("back")));
    assert_eq!(field_val(&p.records[0], "score"), Some(&json!(4)));

    let p = assert_valid("my left foot hurts 3");
    assert_eq!(
        field_val(&p.records[0], "body_zone"),
        Some(&json!("left_foot"))
    );
    assert_eq!(field_val(&p.records[0], "score"), Some(&json!(3)));

    let p = assert_valid("me duele el pecho");
    assert_eq!(field_val(&p.records[0], "body_zone"), Some(&json!("chest")));
    assert_eq!(field_val(&p.records[0], "score"), None);

    let p = assert_valid("me duele la cabeza un 6 desde esta mañana");
    assert_eq!(
        p.records[0].time,
        TimeHint::Ambiguous {
            quote: "desde esta mañana".to_owned()
        }
    );

    let p = assert_valid("headache 5 since this morning");
    assert_eq!(
        p.records[0].time,
        TimeHint::Ambiguous {
            quote: "since this morning".to_owned()
        }
    );

    let p = assert_valid("me duele la cabeza hace 2 horas");
    assert_eq!(field_val(&p.records[0], "score"), None);
    assert_eq!(
        p.records[0].time,
        TimeHint::Ambiguous {
            quote: "hace 2 horas".to_owned()
        }
    );
}

#[test]
fn negations_mixed_multiple_and_uninterpreted_cases() {
    let p = assert_valid("no me duele la cabeza");
    assert!(p.records.is_empty());
    assert_eq!(p.negated, vec!["no me duele la cabeza"]);

    let p = assert_valid("ya no me duele");
    assert!(p.records.is_empty());
    assert_eq!(p.negated, vec!["ya no me duele"]);

    let p = assert_valid("no headache");
    assert!(p.records.is_empty());
    assert_eq!(p.negated, vec!["no headache"]);

    let p = assert_valid("ayer dormí mal pero hoy ya no me duele");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));
    assert_eq!(
        p.records[0].time,
        TimeHint::Ambiguous {
            quote: "ayer".to_owned()
        }
    );
    assert_eq!(p.negated, vec!["hoy ya no me duele"]);

    let p = assert_valid("me duele la cabeza un 6 desde esta mañana y dormí mal");
    assert_eq!(p.records.len(), 2);
    assert_eq!(p.records[0].indicator, Indicator::Symptom);
    assert_eq!(
        p.records[0].time,
        TimeHint::Ambiguous {
            quote: "desde esta mañana".to_owned()
        }
    );
    assert_eq!(p.records[1].indicator, Indicator::Sleep);
    assert_eq!(p.records[1].time, TimeHint::Unspecified);

    let p = assert_valid("pulso 72 temperatura 36.8 SpO₂ 98%");
    assert_eq!(p.records.len(), 3);
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));
    assert_eq!(field_val(&p.records[1], "celsius"), Some(&json!(36.8)));
    assert_eq!(field_val(&p.records[2], "percentage"), Some(&json!(98)));

    let p = assert_valid("tengo heart rate setenta y dos and dolor de right arm 5");
    assert_eq!(p.records.len(), 2);
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));
    assert_eq!(
        field_val(&p.records[1], "body_zone"),
        Some(&json!("right_arm"))
    );
    assert_eq!(field_val(&p.records[1], "score"), Some(&json!(5)));

    let p = assert_valid("TENÍA ÁNIMO 6 y PRESIÓN 120/80");
    assert_eq!(p.records.len(), 2);
    assert_eq!(p.records[0].quote, "TENÍA ÁNIMO 6");
    assert_eq!(p.records[1].quote, "PRESIÓN 120/80");

    let p = assert_valid("  todo va tranquilo en la estación  ");
    assert!(p.records.is_empty());
    assert!(p.negated.is_empty());
    assert_eq!(p.uninterpreted, vec!["todo va tranquilo en la estación"]);

    let p = assert_valid("pulso 72, la escotilla hace ruido");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.uninterpreted, vec!["la escotilla hace ruido"]);

    // Frases de hora con palabras, fechas, rangos, unidades con palabras y negaciones encadenadas
    let p_ocho = assert_valid("me duele la cabeza a las ocho y media");
    assert_eq!(field_val(&p_ocho.records[0], "score"), None);
    assert_eq!(
        p_ocho.records[0].time,
        TimeHint::Ambiguous {
            quote: "a las ocho y media".to_owned()
        }
    );

    let p_date = assert_valid("pulso 06/11/80");
    assert_eq!(field_val(&p_date.records[0], "bpm"), None);

    let p_range = assert_valid("pulso 72-80");
    assert_eq!(field_val(&p_range.records[0], "bpm"), None);

    let p_bpm_words = assert_valid("72 beats per minute");
    assert_eq!(p_bpm_words.records[0].indicator, Indicator::HeartRate);
    assert_eq!(field_val(&p_bpm_words.records[0], "bpm"), Some(&json!(72)));

    let p_deg = assert_valid("37 degrees");
    assert_eq!(p_deg.records[0].indicator, Indicator::BodyTemperature);
    assert_eq!(field_val(&p_deg.records[0], "celsius"), Some(&json!(37)));

    let p_bare_c = assert_valid("0c[");
    assert!(p_bare_c.records.is_empty());

    let p_ni = assert_valid("sin dolor ni fiebre");
    assert!(p_ni.records.is_empty());
    assert_eq!(p_ni.negated, vec!["sin dolor", "ni fiebre"]);

    let p = assert_valid("   ");
    assert!(p.records.is_empty());
    assert!(p.negated.is_empty());
    assert!(p.uninterpreted.is_empty());
}

#[test]
fn out_of_range_numbers_are_never_forced() {
    let p = assert_valid("pulso 0");
    assert_eq!(field_val(&p.records[0], "bpm"), None);

    let p = assert_valid("saturación 150%");
    assert_eq!(field_val(&p.records[0], "percentage"), None);

    let p = assert_valid("ánimo 12");
    assert_eq!(field_val(&p.records[0], "score"), None);

    let p = assert_valid("dormí 30 horas");
    assert_eq!(field_val(&p.records[0], "hours"), None);

    let p = assert_valid("me duele la cabeza un 4.5");
    assert_eq!(field_val(&p.records[0], "body_zone"), Some(&json!("head")));
    assert_eq!(field_val(&p.records[0], "score"), None);
}

#[test]
fn extra_sleep_negation_and_time_cases() {
    let p = assert_valid("I didn't sleep well");
    assert!(p.negated.is_empty());
    assert_eq!(p.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));

    let p = assert_valid("last night slept 6.5 hours");
    assert_eq!(p.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(6.5)));
    assert_eq!(
        p.records[0].time,
        TimeHint::Ambiguous {
            quote: "last night".to_owned()
        }
    );

    let p = assert_valid("dormí mal, 5 horas");
    assert_eq!(p.records.len(), 1);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));
    assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(5)));

    let p = assert_valid("sin dolor");
    assert!(p.records.is_empty());
    assert_eq!(p.negated, vec!["sin dolor"]);

    let p = assert_valid("no tengo fiebre");
    assert!(p.records.is_empty());
    assert_eq!(p.negated, vec!["no tengo fiebre"]);

    let p = assert_valid("headache is gone");
    assert!(p.records.is_empty());
    assert_eq!(p.negated, vec!["headache is gone"]);

    let p = assert_valid("my head doesn't hurt");
    assert!(p.records.is_empty());
    assert_eq!(p.negated, vec!["my head doesn't hurt"]);
}

// ============================================================================
// Regresiones obligatorias para todos los hallazgos de Sol (tabla slm-f0-interprete.md)
// ============================================================================

#[test]
fn regression_local_number_attribution_and_competing_numbers() {
    // 1. "tras 6 ejercicios pulso 72" -> bpm 72 (nunca 6)
    let p = assert_valid("tras 6 ejercicios pulso 72");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::HeartRate);
    assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));
    assert_eq!(field_quote(&p.records[0], "bpm"), Some("72"));

    // Sin número tras el ancla: el 6 previo no se atribuye al pulso
    let p_none = assert_valid("tras 6 ejercicios pulso");
    assert_eq!(p_none.records.len(), 1);
    assert_eq!(field_val(&p_none.records[0], "bpm"), None);

    // Números competidores sin asociación inequívoca -> None + uninterpreted
    let p_comp = assert_valid("pulso 72 o 80");
    assert_eq!(p_comp.records.len(), 1);
    assert_eq!(field_val(&p_comp.records[0], "bpm"), None);
    assert_eq!(p_comp.uninterpreted, vec!["pulso 72 o 80"]);
}

#[test]
fn regression_incompatible_and_unknown_units_yield_none() {
    // 2a. "dormí 20 minutos" -> hours: None (no 20)
    let p_sleep = assert_valid("dormí 20 minutos");
    assert_eq!(p_sleep.records.len(), 1);
    assert_eq!(p_sleep.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p_sleep.records[0], "hours"), None);

    // 2b. "temperatura 98.6 F" -> celsius: None (no 98.6)
    let p_temp = assert_valid("temperatura 98.6 F");
    assert_eq!(p_temp.records.len(), 1);
    assert_eq!(p_temp.records[0].indicator, Indicator::BodyTemperature);
    assert_eq!(field_val(&p_temp.records[0], "celsius"), None);

    // 2c. "glucosa 5,5 mmol/L" -> mg_dl: None (no 5.5)
    let p_gluc = assert_valid("glucosa 5,5 mmol/L");
    assert_eq!(p_gluc.records.len(), 1);
    assert_eq!(p_gluc.records[0].indicator, Indicator::Glucose);
    assert_eq!(field_val(&p_gluc.records[0], "mg_dl"), None);
}

#[test]
fn regression_clock_times_consumed_wholly_and_kept_ambiguous() {
    // 3a. "me duele la cabeza a las 08:05" -> score None, TimeHint::Ambiguous("a las 08:05")
    let p1 = assert_valid("me duele la cabeza a las 08:05");
    assert_eq!(p1.records.len(), 1);
    assert_eq!(p1.records[0].indicator, Indicator::Symptom);
    assert_eq!(field_val(&p1.records[0], "body_zone"), Some(&json!("head")));
    assert_eq!(field_val(&p1.records[0], "score"), None);
    assert_eq!(
        p1.records[0].time,
        TimeHint::Ambiguous {
            quote: "a las 08:05".to_owned()
        }
    );

    // 3b. "me duele la cabeza desde las 8" -> score None, TimeHint::Ambiguous("desde las 8")
    let p2 = assert_valid("me duele la cabeza desde las 8");
    assert_eq!(p2.records.len(), 1);
    assert_eq!(field_val(&p2.records[0], "body_zone"), Some(&json!("head")));
    assert_eq!(field_val(&p2.records[0], "score"), None);
    assert_eq!(
        p2.records[0].time,
        TimeHint::Ambiguous {
            quote: "desde las 8".to_owned()
        }
    );

    // 3c. "me duele la cabeza at 8pm" -> score None, TimeHint::Ambiguous("at 8pm")
    let p3 = assert_valid("me duele la cabeza at 8pm");
    assert_eq!(p3.records.len(), 1);
    assert_eq!(field_val(&p3.records[0], "body_zone"), Some(&json!("head")));
    assert_eq!(field_val(&p3.records[0], "score"), None);
    assert_eq!(
        p3.records[0].time,
        TimeHint::Ambiguous {
            quote: "at 8pm".to_owned()
        }
    );

    // 3d. Reloj malformado "a las 08:99" tampoco filtra 08 ni 99
    let p4 = assert_valid("me duele la cabeza a las 08:99");
    assert_eq!(field_val(&p4.records[0], "score"), None);
    assert_eq!(
        p4.records[0].time,
        TimeHint::Ambiguous {
            quote: "a las 08:99".to_owned()
        }
    );
}

#[test]
fn regression_unsupported_number_expressions_and_signs_are_rejected_wholly() {
    // 4a. "pulso trescientos setenta y dos" -> bpm None (nunca 72)
    let p1 = assert_valid("pulso trescientos setenta y dos");
    assert_eq!(p1.records.len(), 1);
    assert_eq!(p1.records[0].indicator, Indicator::HeartRate);
    assert_eq!(field_val(&p1.records[0], "bpm"), None);

    // 4b. "ánimo -6" -> score None (nunca 6)
    let p2 = assert_valid("ánimo -6");
    assert_eq!(p2.records.len(), 1);
    assert_eq!(p2.records[0].indicator, Indicator::Mood);
    assert_eq!(field_val(&p2.records[0], "score"), None);

    // 4c. "pulso three hundred seventy two" -> bpm None
    let p3 = assert_valid("pulso three hundred seventy two");
    assert_eq!(field_val(&p3.records[0], "bpm"), None);
}

#[test]
fn regression_non_ten_fractions_never_reuse_denominator() {
    // 5a. "ánimo 6/7" -> score None (nunca 7)
    let p1 = assert_valid("ánimo 6/7");
    assert_eq!(p1.records.len(), 1);
    assert_eq!(p1.records[0].indicator, Indicator::Mood);
    assert_eq!(field_val(&p1.records[0], "score"), None);

    // 5b. "dolor de espalda 6/10" -> score 6
    let p2 = assert_valid("dolor de espalda 6/10");
    assert_eq!(field_val(&p2.records[0], "score"), Some(&json!(6)));
    assert_eq!(field_quote(&p2.records[0], "score"), Some("6/10"));
}

#[test]
fn regression_local_negation_scope_and_all_classes() {
    // 6a. "pulso 72 sin dolor" -> conserva pulso 72 y nega "sin dolor"
    let p1 = assert_valid("pulso 72 sin dolor");
    assert_eq!(p1.records.len(), 1);
    assert_eq!(p1.records[0].indicator, Indicator::HeartRate);
    assert_eq!(field_val(&p1.records[0], "bpm"), Some(&json!(72)));
    assert_eq!(p1.negated, vec!["sin dolor"]);

    // 6b. "dormí bien no me duele la cabeza 6" -> Sleep good + negated síntoma
    let p2 = assert_valid("dormí bien no me duele la cabeza 6");
    assert_eq!(p2.records.len(), 1);
    assert_eq!(p2.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p2.records[0], "quality"), Some(&json!("good")));
    assert_eq!(p2.negated, vec!["no me duele la cabeza 6"]);

    // 7a. "no tengo fatiga 6" -> negated (nunca síntoma positivo)
    let p3 = assert_valid("no tengo fatiga 6");
    assert!(p3.records.is_empty());
    assert_eq!(p3.negated, vec!["no tengo fatiga 6"]);

    // 7b. "no medí pulso 72" -> negated (nunca pulso positivo)
    let p4 = assert_valid("no medí pulso 72");
    assert!(p4.records.is_empty());
    assert_eq!(p4.negated, vec!["no medí pulso 72"]);
}

#[test]
fn regression_corrections_and_repetitions_never_drop_silently() {
    // 8. "pulso 72 perdón pulso 82" -> marca como ambiguo (bpm: None + uninterpreted)
    let p = assert_valid("pulso 72 perdón pulso 82");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::HeartRate);
    assert_eq!(field_val(&p.records[0], "bpm"), None);
    assert_eq!(p.uninterpreted, vec!["pulso 72 perdón pulso 82"]);

    // Repetición sin palabra de corrección tampoco elige el primero en silencio
    let p2 = assert_valid("pulso 72 pulso 82");
    assert_eq!(p2.records.len(), 1);
    assert_eq!(field_val(&p2.records[0], "bpm"), None);
    assert_eq!(p2.uninterpreted, vec!["pulso 72 pulso 82"]);
}

#[test]
fn regression_sleep_does_not_absorb_unrelated_durations() {
    // 9a. "ayer dormí mal. hoy trabajé 5 horas" -> horas NO se fusiona al sueño de ayer
    let p = assert_valid("ayer dormí mal. hoy trabajé 5 horas");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));
    assert_eq!(field_val(&p.records[0], "hours"), None);
    assert_eq!(
        p.records[0].time,
        TimeHint::Ambiguous {
            quote: "ayer".to_owned()
        }
    );
    assert_eq!(p.uninterpreted, vec!["hoy trabajé 5 horas"]);

    // 9b. "hoy trabajé 5 horas" -> nunca infiere sueño
    let p2 = assert_valid("hoy trabajé 5 horas");
    assert!(p2.records.is_empty());
    assert_eq!(p2.uninterpreted, vec!["hoy trabajé 5 horas"]);
}

#[test]
fn max_input_chars_limit_returns_empty_proposal() {
    let at_limit = format!("{}pulso 72", " ".repeat(MAX_INPUT_CHARS - 8));
    assert_eq!(at_limit.chars().count(), MAX_INPUT_CHARS);
    let p_ok = assert_valid(&at_limit);
    assert_eq!(p_ok.records.len(), 1);
    assert_eq!(field_val(&p_ok.records[0], "bpm"), Some(&json!(72)));

    let over_limit = "a".repeat(MAX_INPUT_CHARS + 1);
    let p_over = assert_valid(&over_limit);
    assert!(p_over.records.is_empty());
    assert!(p_over.negated.is_empty());
    assert!(p_over.uninterpreted.is_empty());
}

#[test]
fn public_lexicon_helpers_behave_conservatively() {
    // quote_number
    assert_eq!(quote_number("72"), Some(72.0));
    assert_eq!(quote_number("setenta y dos"), Some(72.0));
    assert_eq!(quote_number("ciento setenta y dos"), Some(172.0));
    assert_eq!(quote_number("treinta y seis coma ocho"), Some(36.8));
    assert_eq!(quote_number("36,8 °C"), Some(36.8));
    assert_eq!(quote_number("95 mg/dL"), Some(95.0));
    assert_eq!(quote_number("98.2%"), Some(98.2));
    assert_eq!(quote_number("6/10"), Some(6.0));
    assert_eq!(quote_number("cinco horas y media"), Some(5.5));
    assert_eq!(quote_number("five and a half"), Some(5.5));
    assert_eq!(quote_number("media hora"), Some(0.5));
    assert_eq!(quote_number("pulso 72"), None);
    assert_eq!(quote_number("72 bpm extra"), None);
    assert_eq!(quote_number("-6"), None);
    assert_eq!(quote_number("trescientos setenta y dos"), None);
    assert_eq!(quote_number("6/7"), None);
    assert_eq!(quote_number("20 minutos"), None);
    assert_eq!(quote_number("98.6 F"), None);
    assert_eq!(quote_number("5,5 mmol/L"), None);
    assert_eq!(quote_number("120/80"), None);

    // quote_zone
    assert_eq!(quote_zone("cabeza"), Some(BodyZone::Head));
    assert_eq!(quote_zone("left foot"), Some(BodyZone::LeftFoot));
    assert_eq!(quote_zone("pie izquierdo"), Some(BodyZone::LeftFoot));
    assert_eq!(quote_zone("fatiga"), Some(BodyZone::Other));
    assert_eq!(quote_zone("cabeza y pie izquierdo"), None);
    assert_eq!(quote_zone("dolor"), None);

    // quote_sleep_quality
    assert_eq!(quote_sleep_quality("mal"), Some(SleepQuality::Poor));
    assert_eq!(quote_sleep_quality("bien"), Some(SleepQuality::Good));
    assert_eq!(quote_sleep_quality("regular"), Some(SleepQuality::Fair));
    assert_eq!(
        quote_sleep_quality("no dormí bien"),
        Some(SleepQuality::Poor)
    );
    assert_eq!(
        quote_sleep_quality("I didn't sleep well"),
        Some(SleepQuality::Poor)
    );
    assert_eq!(quote_sleep_quality("no dormí mal"), None);
    assert_eq!(quote_sleep_quality("bien y mal"), None);
    assert_eq!(quote_sleep_quality("6 horas"), None);

    // mentions_indicator
    assert!(mentions_indicator(Indicator::HeartRate, "pulso 72"));
    assert!(mentions_indicator(Indicator::BloodPressure, "120/80"));
    assert!(mentions_indicator(Indicator::Symptom, "me duele la cabeza"));
    assert!(mentions_indicator(Indicator::Symptom, "fatiga"));
    assert!(!mentions_indicator(Indicator::Sleep, "hoy trabajé 5 horas"));
    assert!(!mentions_indicator(Indicator::Mood, "feel dizzy"));
    assert!(!mentions_indicator(Indicator::HeartRate, "6"));
}

#[test]
fn revision2_regressions_attribution_units_time_sleep_negation_correction() {
    // 1. Atribución no atraviesa otro predicado ("pulso desconocido tras 6 flexiones" -> sin bpm)
    let p = assert_valid("pulso desconocido tras 6 flexiones");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::HeartRate);
    assert_eq!(field_val(&p.records[0], "bpm"), None);

    let p = assert_valid("pulso desconocido tras 6");
    assert_eq!(p.records.len(), 1);
    assert_eq!(field_val(&p.records[0], "bpm"), None);

    // 2. Duración previa a otro verbo/predicado ("trabajé 5 horas antes de dormir mal" -> Sleep sin hours)
    let p = assert_valid("trabajé 5 horas antes de dormir mal");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));
    assert_eq!(field_val(&p.records[0], "hours"), None);

    // 3. Unidad desconocida o Fahrenheit (!= ausencia de unidad): sin celsius
    for txt in ["temperatura 98.6 degF", "temperatura 98.6 ℉", "98.6 ℉"] {
        let p = assert_valid(txt);
        assert!(
            p.records.iter().all(|r| field_val(r, "celsius").is_none()),
            "unexpected celsius value for {txt:?}: {p:?}"
        );
    }

    // 4. Reloj incompleto ("me duele la cabeza a las 08:") -> sin score, tiempo ambiguo
    let p = assert_valid("me duele la cabeza a las 08:");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::Symptom);
    assert_eq!(field_val(&p.records[0], "score"), None);
    assert_eq!(
        p.records[0].time,
        TimeHint::Ambiguous {
            quote: "a las 08:".to_owned()
        }
    );

    // 5. Excepción "no dormí bien" solo para esa construcción; no contaminar bien + otra negación
    let p = assert_valid("dormí bien no tuve pesadillas");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("good")));
    assert!(p.negated.is_empty());

    let p = assert_valid("dormí bien sin interrupciones");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("good")));
    assert!(p.negated.is_empty());

    let p = assert_valid("no dormí bien");
    assert_eq!(p.records.len(), 1);
    assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));

    // 6. Modales negativos y contracciones en inglés
    for txt in [
        "I can't measure pulse 72",
        "I cannot measure pulse 72",
        "I won't measure pulse 72",
        "I didn't check pulse 72",
        "I haven't measured pulse 72",
    ] {
        let p = assert_valid(txt);
        assert!(
            p.records.is_empty(),
            "expected no positive records for {txt:?}: {p:?}"
        );
        assert!(
            !p.negated.is_empty(),
            "expected negated mention for {txt:?}: {p:?}"
        );
    }

    // 7. Corrección entre cláusulas contiguas: nunca dos registros positivos
    let p = assert_valid("pulso 72, perdón pulso 82");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::HeartRate);
    assert_eq!(field_val(&p.records[0], "bpm"), None);
    assert_eq!(p.uninterpreted, vec!["pulso 72, perdón pulso 82"]);

    // 8. hours = 0 vuelve a ser válido en capa 1 en sintonía con units::Hours
    let p = assert_valid("dormí 0 horas");
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.records[0].indicator, Indicator::Sleep);
    assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(0)));
}

fn check_proposal_invariants(s: &str) -> Result<(), proptest::test_runner::TestCaseError> {
    let p = interpret(s);
    let issues = p.check(s);
    prop_assert!(issues.is_empty(), "issues: {issues:?}");

    for rec in &p.records {
        prop_assert!(mentions_indicator(rec.indicator, &rec.quote));
        for f in &rec.fields {
            if let (Some(Value::Number(n)), Some(q)) = (&f.value, &f.quote)
                && let Some(num_f64) = n.as_f64()
            {
                prop_assert!(s.contains(q.as_str()));
                prop_assert!(rec.quote.contains(q.as_str()));
                let q_num = quote_number(q);
                prop_assert!(
                    q_num.is_some_and(|v| (v - num_f64).abs() <= 1e-9),
                    "numeric value {num_f64} not backed by quote_number({q:?}) = {q_num:?}"
                );
            }
        }
    }
    Ok(())
}

proptest! {
    #[test]
    fn arbitrary_strings_never_panic_and_pass_check(s in ".*") {
        check_proposal_invariants(&s)?;
    }

    #[test]
    fn long_and_exotic_unicode_strings_never_panic_and_pass_check(
        parts in prop::collection::vec(
            prop_oneof![
                Just("pulso 72 ".to_owned()),
                Just("me duele la cabeza un 6 ".to_owned()),
                Just("dormí cinco horas y media ".to_owned()),
                Just("no me duele ".to_owned()),
                Just("İ\u{0307}ßSpO₂ 98,2% ".to_owned()),
                Just("🤒🔥💤 ".to_owned()),
                ".{0,120}",
            ],
            1..80
        )
    ) {
        let s = parts.concat();
        check_proposal_invariants(&s)?;
    }
}
