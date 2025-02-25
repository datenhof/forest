pub async fn create_backup(api_base_url: &str) -> Result<String, reqwest::Error> {
    let client = reqwest::Client::new();
    let url = format!("{}/database/backup", api_base_url);

    let response = client.get(url).send().await?;
    response.error_for_status_ref()?;
    let body = response.json::<String>().await?;
    Ok(body)
}