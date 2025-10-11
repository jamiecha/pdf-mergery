use std::fs;
use std::path::PathBuf;
use std::collections::BTreeMap;
use lopdf::{Document, Object, ObjectId};
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

    // Load all documents
    let documents: Vec<Document> = pdf_files
        .iter()
        .map(|path| {
            Document::load(path)
                .map_err(|e| format!("Failed to load PDF '{}': {}", path.display(), e))
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Create a new document
    let mut merged_doc = Document::with_version("1.5");
    let mut max_id = 1u32;
    
    // Maps old object IDs to new object IDs for each document
    let mut id_maps: Vec<BTreeMap<ObjectId, ObjectId>> = vec![BTreeMap::new(); documents.len()];
    
    // First pass: Copy all objects and create ID mappings
    for (doc_idx, doc) in documents.iter().enumerate() {
        for (old_id, object) in doc.objects.iter() {
            let new_id = (max_id, 0);
            max_id += 1;
            id_maps[doc_idx].insert(*old_id, new_id);
            merged_doc.objects.insert(new_id, object.clone());
        }
    }
    
    // Second pass: Update all object references
    for (doc_idx, doc) in documents.iter().enumerate() {
        let id_map = &id_maps[doc_idx];
        
        for (old_id, _) in doc.objects.iter() {
            if let Some(&new_id) = id_map.get(old_id) {
                if let Some(object) = merged_doc.objects.get(&new_id).cloned() {
                    let mut object = object;
                    update_references(&mut object, id_map);
                    merged_doc.objects.insert(new_id, object);
                }
            }
        }
    }
    
    // Collect all page references
    let mut all_page_ids = Vec::new();
    for (doc_idx, doc) in documents.iter().enumerate() {
        let pages = doc.get_pages();
        let mut page_list: Vec<_> = pages.into_iter().collect();
        page_list.sort_by(|a, b| a.0.cmp(&b.0));
        
        for (_, old_page_id) in page_list {
            if let Some(&new_page_id) = id_maps[doc_idx].get(&old_page_id) {
                all_page_ids.push(new_page_id);
            }
        }
    }
    
    // Create new page tree
    let pages_id = (max_id, 0);
    max_id += 1;
    
    let page_refs: Vec<Object> = all_page_ids.iter()
        .map(|&id| Object::Reference(id))
        .collect();
    
    let mut pages_dict = lopdf::Dictionary::new();
    pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set("Kids", Object::Array(page_refs));
    pages_dict.set("Count", Object::Integer(all_page_ids.len() as i64));
    
    merged_doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    
    // Update parent reference for all pages
    for &page_id in &all_page_ids {
        if let Some(Object::Dictionary(ref mut page_dict)) = merged_doc.objects.get_mut(&page_id) {
            page_dict.set("Parent", Object::Reference(pages_id));
        }
    }
    
    // Create catalog
    let catalog_id = (max_id, 0);
    max_id += 1;
    
    let mut catalog = lopdf::Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages_id));
    
    merged_doc.objects.insert(catalog_id, Object::Dictionary(catalog));
    
    // Set trailer
    merged_doc.trailer.set("Root", Object::Reference(catalog_id));
    merged_doc.max_id = max_id;
    
    // Save the merged document
    merged_doc.save(&output_path)
        .map_err(|e| format!("Failed to save merged PDF: {}", e))?;

    Ok(format!("PDFs merged into '{}'", output_path.display()))
}

fn update_references(object: &mut Object, id_map: &BTreeMap<ObjectId, ObjectId>) {
    match object {
        Object::Reference(ref mut id) => {
            if let Some(&new_id) = id_map.get(id) {
                *id = new_id;
            }
        }
        Object::Dictionary(ref mut dict) => {
            for (_, value) in dict.iter_mut() {
                update_references(value, id_map);
            }
        }
        Object::Array(ref mut arr) => {
            for item in arr.iter_mut() {
                update_references(item, id_map);
            }
        }
        Object::Stream(ref mut stream) => {
            for (_, value) in stream.dict.iter_mut() {
                update_references(value, id_map);
            }
        }
        _ => {}
    }
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
