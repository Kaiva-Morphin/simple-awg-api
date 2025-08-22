use std::collections::HashMap;
use serde::Serialize;

#[derive(Serialize)]
pub struct Config {
    name: String,
    file: String,
    config: String
}
#[derive(Serialize)]
pub struct PageData {
    configs: Vec<Config>
}

pub async fn set_page(guid: &str, data: &HashMap<String, (String, String)>) {
    if data.is_empty() {
        remove_page(guid).await.ok();
        return;
    }
    let mut configs = vec![];
    for (_id, (n, c)) in data.iter(){
        configs.push(Config{
            name: n.clone(),
            file: format!("{n}.conf"),
            config: c.clone()
        });
    }
    let mut h = handlebars::Handlebars::new();
    h.register_template_file("index", "data/templates/index.hbs").expect("Failed to register index template");
    let contents = h.render("index", &PageData{configs}).unwrap();
    let dir = format!("data/served/{guid}");
    tokio::fs::create_dir_all(&dir).await.ok();
    tokio::fs::write(format!("{dir}/index.html"), contents).await.expect("Failed to write index.html");
}

pub async fn remove_page(guid: &str) -> anyhow::Result<()> {
    let dir = format!("data/served/{guid}");
    tokio::fs::remove_dir_all(dir).await.ok();
    Ok(())
}