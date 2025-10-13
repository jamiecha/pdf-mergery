use std::fs;
use std::path::PathBuf;
use std::collections::BTreeMap;
use lopdf::{Document, Object, ObjectId};
use tauri::command;

#[command]
fn count_pdfs(dir_path: String) -> Result<usize, String> {
    let dir = PathBuf::from(&dir_path);
    if !dir.is_dir() {
        return Err(format!("Directory '{}' does not exist.", dir_path));
    }

    let pdf_count = fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension().and_then(|ext| ext.to_str()) == Some("pdf")
        })
        .count();

    Ok(pdf_count)
}

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
    let mut id_maps: Vec<BTreeMap<ObjectId, ObjectId>> = Vec::with_capacity(documents.len());
    
    // Two-pass per document: build full mapping, then copy with updates
    for doc in &documents {
        let mut id_map = BTreeMap::new();

        // 1) Allocate all new ids up front
        for &old_id in doc.objects.keys() {
            let new_id = (max_id, 0);
            max_id += 1;
            id_map.insert(old_id, new_id);
        }

        // 2) Copy objects with reference fixups using the full map
        for (&old_id, object) in &doc.objects {
            let mut obj = object.clone();
            update_references(&mut obj, &id_map);
            let new_id = id_map[&old_id];
            merged_doc.objects.insert(new_id, obj);
        }

        id_maps.push(id_map);
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

    Ok(output_path.to_string_lossy().to_string())
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
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![merge_pdfs, count_pdfs])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use lopdf::{Document, Object};

    fn create_minimal_pdf() -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.new_object_id();
        let font_id = doc.new_object_id();
        let resources_id = doc.new_object_id();
        let content_id = doc.new_object_id();

        let mut page = lopdf::Dictionary::new();
        page.set("Type", Object::Name(b"Page".to_vec()));
        page.set("Parent", Object::Reference(pages_id));
        page.set("Contents", Object::Reference(content_id));
        page.set("Resources", Object::Reference(resources_id));
        page.set("MediaBox", Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(612), Object::Integer(792)
        ]));

        let mut resources = lopdf::Dictionary::new();
        let mut fonts = lopdf::Dictionary::new();
        fonts.set("F1", Object::Reference(font_id));
        resources.set("Font", Object::Dictionary(fonts));

        let mut font = lopdf::Dictionary::new();
        font.set("Type", Object::Name(b"Font".to_vec()));
        font.set("Subtype", Object::Name(b"Type1".to_vec()));
        font.set("BaseFont", Object::Name(b"Helvetica".to_vec()));

        let content = lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"BT /F1 12 Tf 100 700 Td (Test) Tj ET".to_vec()
        );

        let mut pages = lopdf::Dictionary::new();
        pages.set("Type", Object::Name(b"Pages".to_vec()));
        pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages.set("Count", Object::Integer(1));

        doc.objects.insert(page_id, Object::Dictionary(page));
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        doc.objects.insert(font_id, Object::Dictionary(font));
        doc.objects.insert(resources_id, Object::Dictionary(resources));
        doc.objects.insert(content_id, Object::Stream(content));

        let catalog_id = doc.add_object(lopdf::Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Catalog".to_vec())),
            ("Pages", Object::Reference(pages_id)),
        ]));

        doc.trailer.set("Root", Object::Reference(catalog_id));
        doc
    }

    #[test]
    fn test_merge_pdfs_success() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        let mut pdf1 = create_minimal_pdf();
        let mut pdf2 = create_minimal_pdf();

        pdf1.save(dir_path.join("test1.pdf")).unwrap();
        pdf2.save(dir_path.join("test2.pdf")).unwrap();

        let result = merge_pdfs(dir_path.to_string_lossy().to_string());
        assert!(result.is_ok());

        let output_path = result.unwrap();
        assert!(std::path::Path::new(&output_path).exists());
        
        let merged = Document::load(&output_path).unwrap();
        let pages = merged.get_pages();
        assert_eq!(pages.len(), 2);

        // Verify each page has valid Contents and Resources references
        for (_, page_id) in pages.iter() {
            let page = merged.get_object(*page_id).unwrap();
            if let Object::Dictionary(page_dict) = page {
                // Check Contents reference exists
                if let Ok(contents_ref) = page_dict.get(b"Contents") {
                    match contents_ref {
                        Object::Reference(ref_id) => {
                            assert!(merged.get_object(*ref_id).is_ok(), 
                                "Contents reference should point to valid object");
                        }
                        Object::Array(arr) => {
                            for item in arr {
                                if let Object::Reference(ref_id) = item {
                                    assert!(merged.get_object(*ref_id).is_ok(), 
                                        "Contents array reference should point to valid object");
                                }
                            }
                        }
                        _ => {}
                    }
                }
                
                // Check Resources reference exists
                if let Ok(resources_ref) = page_dict.get(b"Resources") {
                    if let Object::Reference(ref_id) = resources_ref {
                        assert!(merged.get_object(*ref_id).is_ok(), 
                            "Resources reference should point to valid object");
                    }
                }
            }
        }
    }

    #[test]
    fn test_merge_pdfs_directory_not_found() {
        let result = merge_pdfs("/nonexistent/directory".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_merge_pdfs_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        let result = merge_pdfs(dir_path.to_string_lossy().to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No PDF files found"));
    }

    #[test]
    fn test_count_pdfs_success() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        let mut pdf = create_minimal_pdf();
        pdf.save(dir_path.join("test1.pdf")).unwrap();
        pdf.save(dir_path.join("test2.pdf")).unwrap();
        fs::write(dir_path.join("not_a_pdf.txt"), "test").unwrap();

        let result = count_pdfs(dir_path.to_string_lossy().to_string());
        assert_eq!(result.unwrap(), 2);
    }

    #[test]
    fn test_count_pdfs_directory_not_found() {
        let result = count_pdfs("/nonexistent/directory".to_string());
        assert!(result.is_err());
    }
}
