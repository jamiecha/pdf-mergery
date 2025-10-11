use std::fs;
use std::path::PathBuf;
use lopdf::Document;
use tauri::command;

#[command]
fn merge_pdfs(dir_path: String) -> Result<String, String> {
    let dir = PathBuf::from(&dir_path);
    if !dir.is_dir() {
        return Err(format!("Directory '{}' does not exist.", dir_path));
    }

    let mut pdf_files: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension().and_then(|ext| ext.to_str()) == Some("pdf")
        })
        .map(|entry| entry.path())
        .collect();

    if pdf_files.is_empty() {
        return Err("No PDF files found in the directory.".to_string());
    }

    // Sort by modification time
    pdf_files.sort_by_key(|path| {
        path.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    let output_filename = format!("{}.pdf", dir.file_name().unwrap().to_str().unwrap());
    let output_path = dir.parent().unwrap().join(&output_filename);

    // Load the first document as base
    let mut merged_doc = Document::load(&pdf_files[0])
        .map_err(|e| format!("Failed to load first PDF: {}", e))?;

    // Merge remaining documents
    for pdf_path in &pdf_files[1..] {
        let doc = Document::load(pdf_path)
            .map_err(|e| format!("Failed to load PDF '{}': {}", pdf_path.display(), e))?;
        
        // Get the highest object ID in the merged document
        let max_id = merged_doc.objects.len() as u32;
        
        // Add all objects from the current document
        for (obj_id, obj) in doc.objects.iter() {
            let new_id = (obj_id.0 + max_id, obj_id.1);
            merged_doc.objects.insert(new_id, obj.clone());
        }
    }

    // Renumber pages
    merged_doc.renumber_objects();
    merged_doc.compress();

    // Save the merged document
    merged_doc.save(&output_path)
        .map_err(|e| format!("Failed to save merged PDF: {}", e))?;

    Ok(format!("PDFs merged into '{}'", output_path.display()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![merge_pdfs])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
