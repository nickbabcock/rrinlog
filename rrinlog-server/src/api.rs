use chrono::prelude::*;
use serde_json;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub struct Range {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Target {
    pub target: String,
    pub ref_id: String,
    pub _type: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Query {
    pub range: Range,
    pub interval_ms: i32,
    pub max_data_points: i32,
    pub format: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SeriesResponse {
    pub target: String,
    pub datapoints: Vec<[u32; 2]>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Column {
    pub text: String,
    pub _type: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TableResponse {
    pub columns: Vec<Column>,
    pub _type: String,
    pub rows: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Search {
    pub target: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SearchResponse(pub Vec<String>);

#[cfg(test)]
mod tests {
    use serde_json;
    use api::*;

    #[test]
    fn test_search_de() {
        let d = r#"{ "target": "upper_50" }"#;
        let actual: Search = serde_json::from_str(&d).unwrap();
        let expected = Search {
            target: "upper_50".to_string()
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_search_response_ser() {
        let resp = SearchResponse(vec!["A".to_string(), "B".to_string()]);
        let actual = serde_json::to_string(&resp).unwrap();
        assert_eq!(actual, r#"["A","B"]"#.to_string());
    }
}
