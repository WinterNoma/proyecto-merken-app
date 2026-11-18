use crate::api::{ApiError, PreparedPulse};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Deserialize)]
struct DurableVersion {
    version: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "version", content = "action", deny_unknown_fields)]
enum DurablePreparedPulse {
    #[serde(rename = "1")]
    V1(PreparedPulse),
}

pub(crate) fn encode(action: PreparedPulse) -> Result<Vec<u8>, ApiError> {
    serde_json::to_vec(&DurablePreparedPulse::V1(action)).map_err(|_| invalid_action())
}

pub(crate) fn decode(bytes: &[u8]) -> Result<PreparedPulse, ApiError> {
    let header: DurableVersion = serde_json::from_slice(bytes).map_err(|_| invalid_action())?;
    if header.version != "1" {
        return Err(ApiError {
            code: "unsupported_action_version".into(),
            field: Some("action.version".into()),
            message: "The pending pulse requires a different application version".into(),
        });
    }
    match serde_json::from_slice(bytes).map_err(|_| invalid_action())? {
        DurablePreparedPulse::V1(action) => Ok(action),
    }
}

impl TryFrom<pulsys_api::PreparedPulse> for PreparedPulse {
    type Error = ApiError;

    fn try_from(action: pulsys_api::PreparedPulse) -> Result<Self, Self::Error> {
        convert(&action)
    }
}

impl TryFrom<PreparedPulse> for pulsys_api::PreparedPulse {
    type Error = ApiError;

    fn try_from(action: PreparedPulse) -> Result<Self, Self::Error> {
        convert(&action)
    }
}

fn convert<T: Serialize, U: DeserializeOwned>(action: &T) -> Result<U, ApiError> {
    let bytes = serde_json::to_vec(action).map_err(|_| invalid_action())?;
    serde_json::from_slice(&bytes).map_err(|_| invalid_action())
}

fn invalid_action() -> ApiError {
    ApiError {
        code: "invalid_value".into(),
        field: Some("action".into()),
        message: "The prepared pulse has an invalid format".into(),
    }
}
