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

    #[serde(rename = "refId")]
    pub ref_id: String,

    #[serde(rename = "type")]
    pub _type: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Query {
    pub range: Range,

    #[serde(rename = "intervalMs")]
    pub interval_ms: i64,

    #[serde(rename = "maxDataPoints")]
    pub max_data_points: i64,
    pub targets: Vec<Target>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum TargetData {
    Series(Series),
    Table(Table),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Series {
    pub target: String,
    pub datapoints: Vec<[u64; 2]>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Column {
    pub text: String,

    #[serde(rename = "type")]
    pub _type: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Table {
    pub columns: Vec<Column>,

    #[serde(rename = "type")]
    pub _type: String,
    pub rows: Vec<Vec<serde_json::Value>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Search {
    pub target: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SearchResponse(pub Vec<String>);

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct QueryResponse(pub Vec<TargetData>);

#[cfg(test)]
mod tests {
    use serde_json;
    use api::*;

    #[test]
    fn test_search_de() {
        let d = r#"{ "target": "upper_50" }"#;
        let actual: Search = serde_json::from_str(&d).unwrap();
        let expected = Search {
            target: "upper_50".to_string(),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_search_response_ser() {
        let resp = SearchResponse(vec!["A".to_string(), "B".to_string()]);
        let actual = serde_json::to_string(&resp).unwrap();
        assert_eq!(actual, r#"["A","B"]"#.to_string());
    }

    #[test]
    fn test_table_response_ser() {
        let resp = Table {
            columns: vec![
                Column {
                    text: "Name".to_string(),
                    _type: "Text".to_string(),
                },
            ],
            _type: "table".to_string(),
            rows: vec![vec![json!("nick")]],
        };
        let actual = serde_json::to_string(&resp).unwrap();
        assert_eq!(
            actual,
            r#"{"columns":[{"text":"Name","type":"Text"}],"type":"table","rows":[["nick"]]}"#.to_string()
        );
    }

    #[test]
    fn test_query_de() {
        let d = r#"
{
  "panelId": 1,
  "range": {
    "from": "2016-10-31T06:33:44.866Z",
    "to": "2016-10-31T12:33:44.866Z",
    "raw": {
      "from": "now-6h",
      "to": "now"
    }
  },
  "rangeRaw": {
    "from": "now-6h",
    "to": "now"
  },
  "interval": "30s",
  "intervalMs": 30000,
  "targets": [
     { "target": "upper_50", "refId": "A", "type": "timeserie" },
     { "target": "upper_75", "refId": "B", "type": "timeserie" }
  ],
  "format": "json",
  "maxDataPoints": 550
}
"#;
        let actual: Query = serde_json::from_str(&d).unwrap();
        assert_eq!(actual.interval_ms, 30000);
        assert_eq!(actual.max_data_points, 550);
        assert_eq!(
            actual.range,
            Range {
                from: Utc.ymd(2016, 10, 31).and_hms_milli(6, 33, 44, 866),
                to: Utc.ymd(2016, 10, 31).and_hms_milli(12, 33, 44, 866),
            }
        );
        assert_eq!(
            actual.targets,
            vec![
                Target {
                    target: "upper_50".to_string(),
                    ref_id: "A".to_string(),
                    _type: "timeserie".to_string(),
                },
                Target {
                    target: "upper_75".to_string(),
                    ref_id: "B".to_string(),
                    _type: "timeserie".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_query_table_response_ser() {
        let resp = Table {
            columns: vec![
                Column {
                    text: "Name".to_string(),
                    _type: "Text".to_string(),
                },
            ],
            _type: "table".to_string(),
            rows: vec![vec![json!("nick")]],
        };
        let actual = serde_json::to_string(&TargetData::Table(resp)).unwrap();
        assert_eq!(
            actual,
            r#"{"columns":[{"text":"Name","type":"Text"}],"type":"table","rows":[["nick"]]}"#.to_string()
        );
    }

    #[test]
    fn test_query_series_ser() {
        let resp = Series {
            target: "my_target".to_string(),
            datapoints: vec![[861, 1450754160000], [767, 1450754220000]],
        };
        let actual = serde_json::to_string(&TargetData::Series(resp)).unwrap();
        assert_eq!(
            actual,
            r#"{"target":"my_target","datapoints":[[861,1450754160000],[767,1450754220000]]}"#.to_string()
        );
    }
}
