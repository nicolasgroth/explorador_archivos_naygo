// Naygo — proveedor de metadata para PDF (número de páginas, autor, título).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::{MetadataField, MetadataProvider};
use lopdf::Document;
use std::path::Path;

/// Metadata de archivos PDF usando la crate `lopdf` (pura-Rust, MIT). Solo se parsea la estructura
/// del documento (tabla de páginas y diccionario `/Info` del trailer): nunca se renderiza ni se
/// extrae el contenido. Tolerante: un PDF ilegible o corrupto devuelve lo que se haya podido leer
/// (o vacío); nunca panic (el filesystem es hostil).
pub struct PdfMeta;

impl MetadataProvider for PdfMeta {
    fn extensions(&self) -> &'static [&'static str] {
        &["pdf"]
    }

    fn read(&self, path: &Path) -> Vec<MetadataField> {
        // `load` abre y parsea la estructura del PDF (xref + objetos), sin renderizar nada.
        // Si el archivo no existe, no es un PDF válido o está corrupto → vacío.
        let Ok(doc) = Document::load(path) else {
            return Vec::new();
        };

        let mut fields = Vec::new();

        // Número de páginas: `get_pages` recorre el árbol de páginas y devuelve un mapa; su
        // tamaño es el total. Solo se reporta si hay al menos una página.
        let pages = doc.get_pages().len();
        if pages > 0 {
            fields.push(MetadataField {
                label_key: "meta.pages",
                value: pages.to_string(),
            });
        }

        // Diccionario `/Info` del trailer: contiene Author, Title, etc. Es una referencia a un
        // objeto, así que se desreferencia con `get_object`. Puede faltar por completo.
        if let Some(author) = info_text(&doc, b"Author") {
            fields.push(MetadataField {
                label_key: "meta.author",
                value: author,
            });
        }
        if let Some(title) = info_text(&doc, b"Title") {
            fields.push(MetadataField {
                label_key: "meta.title",
                value: title,
            });
        }

        fields
    }
}

/// Lee un campo de texto del diccionario `/Info` del trailer (p. ej. `Author`, `Title`) y lo
/// devuelve decodificado y recortado. Los strings de PDF pueden venir en UTF-16BE (con BOM),
/// UTF-8 o PDFDocEncoding: `decode_text_string` elige la codificación según el BOM. Devuelve
/// `None` si el campo no existe, no se puede decodificar o queda vacío (defensivo; nunca panic).
fn info_text(doc: &Document, key: &[u8]) -> Option<String> {
    // El trailer apunta al diccionario Info por referencia; hay que desreferenciarlo.
    let info_ref = doc.trailer.get(b"Info").ok()?;
    let info_id = info_ref.as_reference().ok()?;
    let info = doc.get_object(info_id).ok()?.as_dict().ok()?;

    let value = info.get(key).ok()?;
    // Un campo de fecha u otro tipo no textual se descarta silenciosamente.
    let decoded = lopdf::decode_text_string(value).ok()?;
    let trimmed = decoded.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_inexistente_da_vacio() {
        let fields = PdfMeta.read(std::path::Path::new(r"C:\no\existe.pdf"));
        assert!(fields.is_empty());
    }

    #[test]
    fn pdf_minimo_reporta_paginas_autor_y_titulo() {
        use lopdf::{Dictionary, Object};

        // PDF mínimo generado en memoria con lopdf: una página + /Info con Author y Title.
        // Se construyen los diccionarios a mano (sin el macro `dictionary!`) para no depender
        // de su ámbito de importación.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("min.pdf");

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        // Una página que referencia al nodo Pages.
        let mut page = Dictionary::new();
        page.set("Type", Object::Name(b"Page".to_vec()));
        page.set("Parent", Object::Reference(pages_id));
        page.set(
            "MediaBox",
            Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]),
        );
        let page_id = doc.add_object(page);

        // Nodo Pages con esa única página.
        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name(b"Pages".to_vec()));
        pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        // Catálogo raíz.
        let mut catalog = Dictionary::new();
        catalog.set("Type", Object::Name(b"Catalog".to_vec()));
        catalog.set("Pages", Object::Reference(pages_id));
        let catalog_id = doc.add_object(catalog);

        // Diccionario Info con autor y título. `text_string` codifica según el contenido:
        // ASCII → PDFDocEncoding literal; con caracteres fuera de ASCII (la "á") → UTF-16BE.
        // Así el ida y vuelta con `decode_text_string` conserva los acentos.
        let mut info = Dictionary::new();
        info.set("Author", lopdf::text_string("Nicolás Groth"));
        info.set("Title", lopdf::text_string("Manual de Naygo"));
        let info_id = doc.add_object(info);

        doc.trailer.set("Root", Object::Reference(catalog_id));
        doc.trailer.set("Info", Object::Reference(info_id));

        doc.save(&p).unwrap();

        let fields = PdfMeta.read(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.pages" && f.value == "1"),
            "debe reportar 1 página: {fields:?}"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.author" && f.value == "Nicolás Groth"),
            "debe reportar el autor: {fields:?}"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.title" && f.value == "Manual de Naygo"),
            "debe reportar el título: {fields:?}"
        );
    }
}
