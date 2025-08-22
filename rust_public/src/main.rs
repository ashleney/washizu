extern crate anyhow;
extern crate prettyplease;
extern crate quote;
extern crate syn;
extern crate walkdir;

use anyhow::{Context, Result};

use syn::visit_mut::{self, VisitMut};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <root-dir>", args[0]);
        std::process::exit(1);
    }
    let root = std::path::PathBuf::from(&args[1]);

    for entry in walkdir::WalkDir::new(&root) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let src = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.to_string_lossy()))?;

            let mut file_syntax: syn::File = syn::parse_file(&src)
                .with_context(|| format!("parsing {}", path.to_string_lossy()))?;

            let mut transformer = MakePublic {};
            transformer.visit_file_mut(&mut file_syntax);

            let new_src = prettyplease::unparse(&file_syntax);

            if new_src != src {
                std::fs::write(path, new_src)
                    .with_context(|| format!("writing {}", path.to_string_lossy()))?;
                println!("updated {}", path.to_string_lossy());
            }
        }
    }

    Ok(())
}

struct MakePublic {}

impl MakePublic {
    fn make_vis_public(vis: &mut syn::Visibility) {
        *vis = syn::Visibility::Public(syn::token::Pub::default());
    }
}

impl VisitMut for MakePublic {
    fn visit_item_fn_mut(&mut self, node: &mut syn::ItemFn) {
        Self::make_vis_public(&mut node.vis);
        visit_mut::visit_item_fn_mut(self, node);
    }

    fn visit_item_struct_mut(&mut self, node: &mut syn::ItemStruct) {
        Self::make_vis_public(&mut node.vis);
        match &mut node.fields {
            syn::Fields::Named(fields) => {
                for f in &mut fields.named {
                    Self::make_vis_public(&mut f.vis);
                }
            }
            syn::Fields::Unnamed(fields) => {
                for f in &mut fields.unnamed {
                    Self::make_vis_public(&mut f.vis);
                }
            }
            syn::Fields::Unit => {}
        }
        visit_mut::visit_item_struct_mut(self, node);
    }

    fn visit_item_impl_mut(&mut self, node: &mut syn::ItemImpl) {
        let is_trait_impl = node.trait_.is_some();
        if !is_trait_impl {
            for impl_item in &mut node.items {
                if let syn::ImplItem::Fn(m) = impl_item {
                    Self::make_vis_public(&mut m.vis);
                }
            }
        }
        visit_mut::visit_item_impl_mut(self, node);
    }

    fn visit_item_mod_mut(&mut self, node: &mut syn::ItemMod) {
        if let Some((_brace, items)) = &mut node.content {
            for item in items.iter_mut() {
                self.visit_item_mut(item);
            }
        }
    }
}
