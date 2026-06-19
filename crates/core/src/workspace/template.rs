// Naygo — plantillas de layout y store de recientes/favoritos.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Una `LayoutTemplate` describe una disposición nombrada (qué paneles y cómo se
//! reparten). Hay built-ins (código) y plantillas del usuario (persistidas). El
//! `TemplateStore` agrega los favoritos y la lista de recientes.

use crate::workspace::layout::SplitDir;
use crate::workspace::PanePurpose;
use serde::{Deserialize, Serialize};

/// De dónde arranca un panel `Files` de una plantilla.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateDir {
    /// El home del usuario.
    Home,
    /// Una ruta fija.
    Fixed(String),
}

/// Un panel descrito por una plantilla (tipo + carpeta inicial si es Files).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplatePane {
    pub purpose: PanePurpose,
    /// Solo relevante para `Files`.
    pub dir: TemplateDir,
}

/// Una disposición nombrada. Los built-in se construyen en código; los del usuario
/// se serializan en `templates.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutTemplate {
    pub name: String,
    pub builtin: bool,
    pub favorite: bool,
    /// Paneles que crea la plantilla, en orden.
    pub panes: Vec<TemplatePane>,
    /// Cómo se reparten visualmente (los índices de hoja referencian `panes`).
    pub layout: LayoutShape,
}

/// La forma del layout descrita por índices a `panes` (no por PaneId, porque la
/// plantilla es previa a la creación de los paneles). La UI/Workspace la
/// materializa creando los paneles y mapeando índice→PaneId.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LayoutShape {
    /// Hoja: el panel `panes[idx]`.
    Leaf(usize),
    /// Grupo de pestañas: los paneles `panes[idx]` apilados, `active` (en 0) es el visible.
    Tabs { members: Vec<usize>, active: usize },
    Split {
        dir: SplitDir,
        fraction: f32,
        first: Box<LayoutShape>,
        second: Box<LayoutShape>,
    },
}

impl LayoutTemplate {
    /// Minimalista: un solo panel de archivos.
    pub fn minimalista() -> Self {
        LayoutTemplate {
            name: "Minimalista".into(),
            builtin: true,
            favorite: false,
            panes: vec![TemplatePane {
                purpose: PanePurpose::Files,
                dir: TemplateDir::Home,
            }],
            layout: LayoutShape::Leaf(0),
        }
    }

    /// Clásico: árbol | archivos | inspector.
    pub fn clasico() -> Self {
        LayoutTemplate {
            name: "Clásico".into(),
            builtin: true,
            favorite: false,
            panes: vec![
                TemplatePane {
                    purpose: PanePurpose::Tree,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Files,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Inspector,
                    dir: TemplateDir::Home,
                },
            ],
            layout: LayoutShape::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.22,
                first: Box::new(LayoutShape::Leaf(0)),
                second: Box::new(LayoutShape::Split {
                    dir: SplitDir::Horizontal,
                    fraction: 0.74,
                    first: Box::new(LayoutShape::Leaf(1)),
                    second: Box::new(LayoutShape::Leaf(2)),
                }),
            },
        }
    }

    /// Dual-pane: árbol | archivos A | archivos B | inspector. (Default de la app.)
    pub fn dual_pane() -> Self {
        LayoutTemplate {
            name: "Dual-pane".into(),
            builtin: true,
            favorite: false,
            panes: vec![
                TemplatePane {
                    purpose: PanePurpose::Tree,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Files,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Files,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Inspector,
                    dir: TemplateDir::Home,
                },
            ],
            layout: LayoutShape::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.18,
                first: Box::new(LayoutShape::Leaf(0)),
                second: Box::new(LayoutShape::Split {
                    dir: SplitDir::Horizontal,
                    fraction: 0.4,
                    first: Box::new(LayoutShape::Leaf(1)),
                    second: Box::new(LayoutShape::Split {
                        dir: SplitDir::Horizontal,
                        fraction: 0.66,
                        first: Box::new(LayoutShape::Leaf(2)),
                        second: Box::new(LayoutShape::Leaf(3)),
                    }),
                }),
            },
        }
    }

    /// Disposición de PRIMERA EJECUCIÓN (la "clásica completa"): árbol a la
    /// izquierda, dos paneles de archivos en el centro, y a la derecha una columna
    /// con Propiedades (Inspector) arriba y Vista previa (Preview) abajo. No es una
    /// entrada del menú de plantillas (no está en `builtins`): la usa `main` para
    /// armar el arranque por defecto cuando no hay una sesión guardada.
    pub fn primera_ejecucion() -> Self {
        LayoutTemplate {
            name: "Clásico completo".into(),
            builtin: true,
            favorite: false,
            // 0: árbol | 1: archivos A | 2: archivos B | 3: propiedades | 4: vista previa
            panes: vec![
                TemplatePane {
                    purpose: PanePurpose::Tree,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Files,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Files,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Inspector,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Preview,
                    dir: TemplateDir::Home,
                },
            ],
            // árbol (0.18) | [ archivos A | archivos B ] (centro) | [ props / preview ] (derecha)
            layout: LayoutShape::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.18,
                first: Box::new(LayoutShape::Leaf(0)),
                second: Box::new(LayoutShape::Split {
                    dir: SplitDir::Horizontal,
                    fraction: 0.4,
                    first: Box::new(LayoutShape::Leaf(1)),
                    second: Box::new(LayoutShape::Split {
                        dir: SplitDir::Horizontal,
                        fraction: 0.62,
                        first: Box::new(LayoutShape::Leaf(2)),
                        // Columna derecha: propiedades arriba, vista previa abajo.
                        second: Box::new(LayoutShape::Split {
                            dir: SplitDir::Vertical,
                            fraction: 0.5,
                            first: Box::new(LayoutShape::Leaf(3)),
                            second: Box::new(LayoutShape::Leaf(4)),
                        }),
                    }),
                }),
            },
        }
    }

    /// Power-user: tres paneles de archivos lado a lado + inspector.
    pub fn power_user() -> Self {
        LayoutTemplate {
            name: "Power-user".into(),
            builtin: true,
            favorite: false,
            panes: vec![
                TemplatePane {
                    purpose: PanePurpose::Files,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Files,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Files,
                    dir: TemplateDir::Home,
                },
                TemplatePane {
                    purpose: PanePurpose::Inspector,
                    dir: TemplateDir::Home,
                },
            ],
            layout: LayoutShape::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.3,
                first: Box::new(LayoutShape::Leaf(0)),
                second: Box::new(LayoutShape::Split {
                    dir: SplitDir::Horizontal,
                    fraction: 0.43,
                    first: Box::new(LayoutShape::Leaf(1)),
                    second: Box::new(LayoutShape::Split {
                        dir: SplitDir::Horizontal,
                        fraction: 0.6,
                        first: Box::new(LayoutShape::Leaf(2)),
                        second: Box::new(LayoutShape::Leaf(3)),
                    }),
                }),
            },
        }
    }

    /// Todas las plantillas built-in, en el orden en que se muestran.
    pub fn builtins() -> Vec<LayoutTemplate> {
        vec![
            Self::minimalista(),
            Self::clasico(),
            Self::dual_pane(),
            Self::power_user(),
        ]
    }
}

/// Un uso reciente de una plantilla (nombre + timestamp inyectado por la UI).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecentUse {
    pub name: String,
    /// Segundos epoch; lo inyecta la capa ui (core no llama a `SystemTime::now`).
    pub at: u64,
}

/// Tope de entradas en la lista de recientes.
const MAX_RECENTS: usize = 8;

/// Plantillas del usuario + recientes. Lo que se persiste en `templates.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TemplateStore {
    /// Plantillas creadas por el usuario (builtin = false).
    pub user: Vec<LayoutTemplate>,
    /// Usos recientes, del más nuevo al más viejo.
    pub recents: Vec<RecentUse>,
}

impl TemplateStore {
    /// Registra el uso de una plantilla: la pone al frente de recientes (sin
    /// duplicar) y respeta el tope. `at` es el timestamp inyectado por la ui.
    pub fn record_use(&mut self, name: &str, at: u64) {
        self.recents.retain(|r| r.name != name);
        self.recents.insert(
            0,
            RecentUse {
                name: name.to_string(),
                at,
            },
        );
        self.recents.truncate(MAX_RECENTS);
    }

    /// Marca/desmarca como favorita una plantilla del usuario por nombre.
    pub fn set_favorite(&mut self, name: &str, favorite: bool) {
        if let Some(t) = self.user.iter_mut().find(|t| t.name == name) {
            t.favorite = favorite;
        }
    }

    /// Agrega una plantilla del usuario (la fuerza a builtin=false).
    pub fn add_user(&mut self, mut t: LayoutTemplate) {
        t.builtin = false;
        // Si ya existe una con el mismo nombre, la reemplaza.
        self.user.retain(|x| x.name != t.name);
        self.user.push(t);
    }

    /// Borra una plantilla del usuario por nombre.
    pub fn remove_user(&mut self, name: &str) {
        self.user.retain(|t| t.name != name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_tienen_las_cuatro() {
        let names: Vec<_> = LayoutTemplate::builtins()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(
            names,
            vec!["Minimalista", "Clásico", "Dual-pane", "Power-user"]
        );
    }

    #[test]
    fn primera_ejecucion_es_la_clasica_completa() {
        // Arranque clásico: árbol + 2 archivos + propiedades + vista previa = 5 paneles.
        let t = LayoutTemplate::primera_ejecucion();
        assert_eq!(t.panes.len(), 5);
        let count = |p: PanePurpose| t.panes.iter().filter(|x| x.purpose == p).count();
        assert_eq!(count(PanePurpose::Tree), 1, "un árbol");
        assert_eq!(count(PanePurpose::Files), 2, "dos paneles de archivos");
        assert_eq!(count(PanePurpose::Inspector), 1, "propiedades");
        assert_eq!(count(PanePurpose::Preview), 1, "vista previa");
        // No está en el menú de plantillas (builtins): es solo para el arranque por defecto.
        assert!(!builtins().iter().any(|b| b.name == t.name));
    }

    /// Helper local: las built-in del menú (las que sí aparecen como opciones).
    fn builtins() -> Vec<LayoutTemplate> {
        LayoutTemplate::builtins()
    }

    #[test]
    fn minimalista_es_un_solo_files() {
        let t = LayoutTemplate::minimalista();
        assert_eq!(t.panes.len(), 1);
        assert_eq!(t.panes[0].purpose, PanePurpose::Files);
        assert_eq!(t.layout, LayoutShape::Leaf(0));
    }

    #[test]
    fn record_use_pone_al_frente_sin_duplicar() {
        let mut s = TemplateStore::default();
        s.record_use("Dual-pane", 100);
        s.record_use("Power-user", 200);
        s.record_use("Dual-pane", 300); // re-uso: sube al frente, no duplica
        assert_eq!(s.recents.len(), 2);
        assert_eq!(s.recents[0].name, "Dual-pane");
        assert_eq!(s.recents[0].at, 300);
    }

    #[test]
    fn recientes_respeta_el_tope() {
        let mut s = TemplateStore::default();
        for i in 0..20 {
            s.record_use(&format!("t{i}"), i as u64);
        }
        assert_eq!(s.recents.len(), MAX_RECENTS);
        assert_eq!(s.recents[0].name, "t19");
    }

    #[test]
    fn add_user_fuerza_no_builtin_y_reemplaza_por_nombre() {
        let mut s = TemplateStore::default();
        let mut t = LayoutTemplate::minimalista();
        t.name = "Mía".into();
        t.builtin = true; // debe forzarse a false
        s.add_user(t.clone());
        s.add_user(t); // mismo nombre: reemplaza, no duplica
        assert_eq!(s.user.len(), 1);
        assert!(!s.user[0].builtin);
    }

    #[test]
    fn set_favorite_marca_la_del_usuario() {
        let mut s = TemplateStore::default();
        let mut t = LayoutTemplate::minimalista();
        t.name = "Mía".into();
        s.add_user(t);
        s.set_favorite("Mía", true);
        assert!(s.user[0].favorite);
    }
}
