use serde::{Deserialize, Serialize};

impl From<&str> for BallotSide {
    fn from(s: &str) -> Self {
        match s {
            "front" => BallotSide::Front,
            "back" => BallotSide::Back,
            _ => panic!("Invalid ballot side: {}", s),
        }
    }
}

impl<'de> Deserialize<'de> for BallotSide {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(s.as_str().into())
    }
}

impl Serialize for BallotSide {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            BallotSide::Front => serializer.serialize_str("front"),
            BallotSide::Back => serializer.serialize_str("back"),
        }
    }
}

// Defines a new type that wraps a String for use as an ID.
macro_rules! idtype {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            #[allow(dead_code)]
            pub fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

pub(crate) use idtype;

use crate::ballot_card::BallotSide;
