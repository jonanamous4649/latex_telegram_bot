use anyhow::Result;

// 'serde::Serialize' converts Rust structs to JSON
// Tera template engine uses JSON to fill templates
use serde::Serialize;

// gets the current date/time in your local timezone
use chrono::Local;

// struct for Metric e.g (Revenue, Value, Unit of Measurement)
#[derive(Serialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

// struct for report data
#[derive(Serialize)]
pub struct ReportData {
    pub report_title: String,
    pub generation_date: String,
    // 'Vec<>' is a vectro (mutable array) of Metric structs
    pub metrics: Vec<Metric>,
    pub analysis_text: String,
    pub include_table: bool,
    pub table_columns: Vec<String>,
    pub table_data: Vec<Vec<String>>,
}

// fetch data fn
pub async fn fetch_data() -> Result<ReportData> {
    
    // ======================================================
    // EXAMPLE DATA
    // ======================================================
    println!(" Creating example metrics...");

    // Create vector of metrics using
    let metrics = vec![
        
        Metric {
            name: "Total Revenue".to_string(),
            value: 125000.50,
            unit: "USD".to_string(),
        },

        Metric {
            name: "Growth Rate".to_string(),
            value: 15.3,
            unit: "%".to_string(),
        },

        Metric {
            name: "Active Users".to_string(),
            value: 5420.0,
            unit: "users".to_string()
        },
    ];

    // ======================================================
    // BUILD REPORT DATA
    // ======================================================
    println!(" Building report structure...");
    
    // Local::now() returns a DateTime<Local> object
    let now = Local::now().format("%Y-%m-%d %H:%M").to_string();

    // Create table data as a 2D vector
    let table_data = vec![
        vec![
            "Month".to_string(),
            "Sales".to_string(),
            "Profit".to_string()
        ],
        vec![
            "October".to_string(),
            "85000".to_string(),
            "12000".to_string()
        ],
        vec![
            "November".to_string(),
            "92000".to_string(),
            "15000".to_string()
        ],
        vec![
            "December".to_string(),
            "110000".to_string(),
            "22000".to_string()
        ],
    ];

    // Construct final ReportData struct -> gets passed to template engine
    let report_data = ReportData {
        report_title: "Automated Business Report".to_string(),
        generation_date: now,
        metrics: metrics,
        analysis_text: "Q4 shows strong performance with 15% growth \
                        driven by new product liens and holiday sales.".to_string(),
        include_table: true,
        table_columns: vec![
            "|c".to_string(),
            "c".to_string(),
            "c|".to_string()
        ],
        table_data: table_data,
    };
    
    Ok(report_data)
}