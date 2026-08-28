use std::path::Path;

use syn::visit::Visit;

pub fn assert_single_token_attachment(path: &Path, source: &str) {
    struct AttachmentVisitor<'a> {
        path: &'a Path,
    }

    impl AttachmentVisitor<'_> {
        fn check(&self, attributes: &[syn::Attribute]) {
            assert!(
                attributes
                    .iter()
                    .filter(|attribute| attribute.path().is_ident("tok"))
                    .count()
                    <= 1,
                "{} contains a grammar site with multiple #[tok] attachments",
                self.path.display()
            );
        }
    }

    impl<'ast> Visit<'ast> for AttachmentVisitor<'_> {
        fn visit_field(&mut self, field: &'ast syn::Field) {
            self.check(&field.attrs);
            syn::visit::visit_field(self, field);
        }

        fn visit_variant(&mut self, variant: &'ast syn::Variant) {
            self.check(&variant.attrs);
            syn::visit::visit_variant(self, variant);
        }

        fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
            self.check(&item.attrs);
            syn::visit::visit_item_struct(self, item);
        }

        fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
            self.check(&item.attrs);
            syn::visit::visit_item_enum(self, item);
        }
    }

    let parsed = syn::parse_file(source).unwrap();
    AttachmentVisitor { path }.visit_file(&parsed);
}
