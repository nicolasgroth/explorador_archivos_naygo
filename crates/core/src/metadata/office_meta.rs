// Naygo — proveedor de metadata para Office moderno (docx/xlsx/pptx) leyendo docProps del zip.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::{MetadataField, MetadataProvider};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::io::Read;
use std::path::Path;

/// Metadata de los formatos Office modernos (OOXML): docx, xlsx y pptx. Todos son un ZIP con la
/// misma estructura interna `docProps/`, así que un solo proveedor los cubre. Se leen solo un par
/// de XML pequeños dentro del zip (nunca todo el documento). Tolerante: cualquier error de zip o de
/// XML devuelve lo que se haya podido leer (o vacío); nunca panic (el filesystem es hostil).
pub struct OfficeMeta;

impl MetadataProvider for OfficeMeta {
    fn extensions(&self) -> &'static [&'static str] {
        &["docx", "xlsx", "pptx"]
    }

    fn read(&self, path: &Path) -> Vec<MetadataField> {
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_ascii_lowercase(),
            None => return Vec::new(),
        };

        // Abrir el contenedor zip. Si el archivo no es un zip válido → vacío.
        let Ok(file) = std::fs::File::open(path) else {
            return Vec::new();
        };
        let Ok(mut archive) = zip::ZipArchive::new(file) else {
            return Vec::new();
        };

        let mut fields = Vec::new();

        // Propiedades comunes: autor, título y fecha de modificación (docProps/core.xml).
        if let Some(xml) = read_entry(&mut archive, "docProps/core.xml") {
            if let Some(author) = first_text(&xml, "creator") {
                fields.push(MetadataField {
                    label_key: "meta.author",
                    value: author,
                });
            }
            if let Some(title) = first_text(&xml, "title") {
                fields.push(MetadataField {
                    label_key: "meta.title",
                    value: title,
                });
            }
            if let Some(modified) = first_text(&xml, "modified") {
                // La fecha viene en ISO 8601 (p. ej. "2026-07-02T10:30:00Z"); mostramos solo el día.
                let day = modified.split('T').next().unwrap_or(&modified).to_string();
                if !day.is_empty() {
                    fields.push(MetadataField {
                        label_key: "meta.modified",
                        value: day,
                    });
                }
            }
        }

        // Contador propio de cada tipo.
        match ext.as_str() {
            "docx" => {
                if let Some(xml) = read_entry(&mut archive, "docProps/app.xml") {
                    if let Some(words) = first_text(&xml, "Words") {
                        fields.push(MetadataField {
                            label_key: "meta.word_count",
                            value: words,
                        });
                    }
                }
            }
            "pptx" => {
                if let Some(xml) = read_entry(&mut archive, "docProps/app.xml") {
                    if let Some(slides) = first_text(&xml, "Slides") {
                        fields.push(MetadataField {
                            label_key: "meta.slide_count",
                            value: slides,
                        });
                    }
                }
            }
            "xlsx" => {
                if let Some(xml) = read_entry(&mut archive, "xl/workbook.xml") {
                    let count = count_elements(&xml, "sheet");
                    if count > 0 {
                        fields.push(MetadataField {
                            label_key: "meta.sheet_count",
                            value: count.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }

        fields
    }
}

/// Lee una entrada del zip por su nombre interno y devuelve su contenido como texto UTF-8. Devuelve
/// `None` si la entrada no existe o no se puede leer (tolerante; nunca panic).
fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf).ok()?;
    Some(buf)
}

/// Busca el primer elemento cuyo nombre local sea `local` (ignorando el prefijo de namespace) y
/// devuelve el texto que contiene, ya recortado. `None` si no aparece o si viene vacío. El parseo
/// es por eventos y defensivo: cualquier error de XML corta la búsqueda y devuelve lo hallado.
fn first_text(xml: &str, local: &str) -> Option<String> {
    let mut reader = Reader::from_reader(xml.as_bytes());
    let mut buf = Vec::new();
    let mut inside = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.local_name().as_ref() == local.as_bytes() {
                    inside = true;
                }
            }
            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == local.as_bytes() {
                    inside = false;
                }
            }
            Ok(Event::Text(e)) if inside => {
                if let Ok(text) = e.decode() {
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Cuenta cuántos elementos tienen el nombre local `local` (ignorando el prefijo de namespace),
/// contando tanto los abiertos (`<sheet>`) como los vacíos (`<sheet .../>`). Defensivo: cualquier
/// error de XML corta el conteo y devuelve lo acumulado.
fn count_elements(xml: &str, local: &str) -> usize {
    let mut reader = Reader::from_reader(xml.as_bytes());
    let mut buf = Vec::new();
    let mut count = 0usize;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.local_name().as_ref() == local.as_bytes() {
                    count += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Escribe un zip mínimo con las entradas dadas (nombre interno → contenido) y devuelve su ruta.
    fn write_zip(
        dir: &std::path::Path,
        file_name: &str,
        entries: &[(&str, &str)],
    ) -> std::path::PathBuf {
        let p = dir.join(file_name);
        let f = std::fs::File::create(&p).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, content) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        p
    }

    #[test]
    fn docx_reporta_autor_titulo_y_palabras() {
        let dir = tempfile::tempdir().unwrap();
        let core_xml = r#"<?xml version="1.0"?><cp:coreProperties xmlns:cp="x" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/"><dc:creator>Nicolás Groth</dc:creator><dc:title>Informe</dc:title><dcterms:modified>2026-07-02T10:30:00Z</dcterms:modified></cp:coreProperties>"#;
        let app_xml = r#"<?xml version="1.0"?><Properties><Words>1234</Words></Properties>"#;
        let p = write_zip(
            dir.path(),
            "x.docx",
            &[
                ("docProps/core.xml", core_xml),
                ("docProps/app.xml", app_xml),
            ],
        );
        let fields = OfficeMeta.read(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.author" && f.value == "Nicolás Groth"),
            "debe reportar el autor: {fields:?}"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.title" && f.value == "Informe"),
            "debe reportar el título: {fields:?}"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.modified" && f.value == "2026-07-02"),
            "debe reportar la fecha de modificación recortada al día: {fields:?}"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.word_count" && f.value == "1234"),
            "debe reportar el conteo de palabras: {fields:?}"
        );
    }

    #[test]
    fn pptx_reporta_diapositivas() {
        let dir = tempfile::tempdir().unwrap();
        let app_xml = r#"<?xml version="1.0"?><Properties><Slides>7</Slides></Properties>"#;
        let p = write_zip(dir.path(), "x.pptx", &[("docProps/app.xml", app_xml)]);
        let fields = OfficeMeta.read(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.slide_count" && f.value == "7"),
            "debe reportar el conteo de diapositivas: {fields:?}"
        );
    }

    #[test]
    fn xlsx_cuenta_las_hojas() {
        let dir = tempfile::tempdir().unwrap();
        // Tres hojas, tal como aparecen en xl/workbook.xml (elementos <sheet .../> vacíos).
        let workbook = r#"<?xml version="1.0"?><workbook xmlns:r="x"><sheets><sheet name="Hoja1" sheetId="1" r:id="rId1"/><sheet name="Hoja2" sheetId="2" r:id="rId2"/><sheet name="Hoja3" sheetId="3" r:id="rId3"/></sheets></workbook>"#;
        let p = write_zip(dir.path(), "x.xlsx", &[("xl/workbook.xml", workbook)]);
        let fields = OfficeMeta.read(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.sheet_count" && f.value == "3"),
            "debe contar 3 hojas: {fields:?}"
        );
    }

    #[test]
    fn archivo_no_zip_da_vacio_sin_panic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("roto.docx");
        std::fs::write(&p, b"esto no es un zip").unwrap();
        assert!(OfficeMeta.read(&p).is_empty());
    }

    #[test]
    fn inexistente_da_vacio() {
        assert!(OfficeMeta
            .read(std::path::Path::new(r"C:\no\existe.xlsx"))
            .is_empty());
    }

    #[test]
    fn zip_truncado_da_vacio_sin_panic() {
        // Un docx válido cortado a la mitad: el directorio central del zip queda incompleto.
        let dir = tempfile::tempdir().unwrap();
        let core_xml = r#"<?xml version="1.0"?><cp:coreProperties xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Nicolás</dc:creator></cp:coreProperties>"#;
        let ok = write_zip(dir.path(), "ok.docx", &[("docProps/core.xml", core_xml)]);
        let bytes = std::fs::read(&ok).unwrap();
        let p = dir.path().join("mitad.docx");
        std::fs::write(&p, &bytes[..bytes.len() / 2]).unwrap();
        assert!(OfficeMeta.read(&p).is_empty());
    }

    #[test]
    fn zip_sin_docprops_da_vacio() {
        // Zip válido pero sin las entradas esperadas (falta docProps/core.xml y app.xml).
        let dir = tempfile::tempdir().unwrap();
        let p = write_zip(dir.path(), "x.docx", &[("otra/cosa.txt", "hola")]);
        assert!(OfficeMeta.read(&p).is_empty());
    }

    #[test]
    fn xml_malformado_no_paniquea_y_rescata_lo_legible() {
        // XML cortado a mitad de un TAG después del autor: el parseo por eventos devuelve lo
        // hallado antes del error y no paniquea. (Si el corte fuera a mitad del TEXTO de un
        // elemento, quick-xml igual entrega ese texto parcial antes del EOF: es tolerante.)
        let dir = tempfile::tempdir().unwrap();
        let core_xml = r#"<?xml version="1.0"?><cp:coreProperties xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Nicolás</dc:creator><dc:tit"#;
        let p = write_zip(dir.path(), "x.docx", &[("docProps/core.xml", core_xml)]);
        let fields = OfficeMeta.read(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.author" && f.value == "Nicolás"),
            "debe rescatar el autor aunque el XML esté cortado: {fields:?}"
        );
        assert!(
            !fields.iter().any(|f| f.label_key == "meta.title"),
            "el tag del título quedó incompleto y no se reporta: {fields:?}"
        );
    }

    #[test]
    fn xml_basura_no_reporta_campos_ni_paniquea() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_zip(
            dir.path(),
            "x.docx",
            &[("docProps/core.xml", "<<<no xml>>>")],
        );
        let fields = OfficeMeta.read(&p);
        assert!(
            !fields
                .iter()
                .any(|f| f.label_key == "meta.author" || f.label_key == "meta.title"),
            "XML basura no produce campos: {fields:?}"
        );
    }

    #[test]
    fn entrada_core_xml_no_utf8_da_vacio_sin_panic() {
        // docProps/core.xml existe pero con bytes que no son UTF-8 válido.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.docx");
        let f = std::fs::File::create(&p).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("docProps/core.xml", opts).unwrap();
        zip.write_all(&[0xFF, 0xFE, 0x00, 0x80, 0x81]).unwrap();
        zip.finish().unwrap();
        assert!(OfficeMeta.read(&p).is_empty());
    }

    #[test]
    fn campos_vacios_no_se_reportan() {
        let dir = tempfile::tempdir().unwrap();
        let core_xml = r#"<?xml version="1.0"?><cp:coreProperties xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>   </dc:creator><dc:title></dc:title></cp:coreProperties>"#;
        let p = write_zip(dir.path(), "x.docx", &[("docProps/core.xml", core_xml)]);
        let fields = OfficeMeta.read(&p);
        assert!(
            fields.is_empty(),
            "campos vacíos o de solo espacios no se reportan: {fields:?}"
        );
    }

    #[test]
    fn docx_sin_app_xml_reporta_core_pero_no_palabras() {
        let dir = tempfile::tempdir().unwrap();
        let core_xml = r#"<?xml version="1.0"?><cp:coreProperties xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Nicolás</dc:creator></cp:coreProperties>"#;
        let p = write_zip(dir.path(), "x.docx", &[("docProps/core.xml", core_xml)]);
        let fields = OfficeMeta.read(&p);
        assert!(fields.iter().any(|f| f.label_key == "meta.author"));
        assert!(
            !fields.iter().any(|f| f.label_key == "meta.word_count"),
            "sin app.xml no hay conteo de palabras: {fields:?}"
        );
    }

    #[test]
    fn xlsx_sin_hojas_no_reporta_sheet_count() {
        let dir = tempfile::tempdir().unwrap();
        let workbook = r#"<?xml version="1.0"?><workbook><sheets></sheets></workbook>"#;
        let p = write_zip(dir.path(), "x.xlsx", &[("xl/workbook.xml", workbook)]);
        let fields = OfficeMeta.read(&p);
        assert!(
            !fields.iter().any(|f| f.label_key == "meta.sheet_count"),
            "un libro sin hojas no reporta sheet_count: {fields:?}"
        );
    }

    #[test]
    fn extension_en_mayusculas_funciona() {
        // El match de extensión se hace en lowercase: "X.DOCX" debe comportarse como docx.
        let dir = tempfile::tempdir().unwrap();
        let app_xml = r#"<?xml version="1.0"?><Properties><Words>42</Words></Properties>"#;
        let p = write_zip(dir.path(), "X.DOCX", &[("docProps/app.xml", app_xml)]);
        let fields = OfficeMeta.read(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.word_count" && f.value == "42"),
            "extensión en mayúsculas sigue siendo docx: {fields:?}"
        );
    }

    #[test]
    fn archivo_sin_extension_da_vacio() {
        let dir = tempfile::tempdir().unwrap();
        let core_xml = r#"<?xml version="1.0"?><cp:coreProperties xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Nicolás</dc:creator></cp:coreProperties>"#;
        let p = write_zip(
            dir.path(),
            "sin_extension",
            &[("docProps/core.xml", core_xml)],
        );
        assert!(OfficeMeta.read(&p).is_empty());
    }

    #[test]
    fn extensiones_declaradas() {
        assert_eq!(OfficeMeta.extensions(), &["docx", "xlsx", "pptx"]);
    }
}
