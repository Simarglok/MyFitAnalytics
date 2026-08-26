use mfa_contracts::ModuleId;
use rfd::FileDialog;
use std::path::PathBuf;

pub trait DialogPort: Send + Sync {
    fn pick_workspace_root(&self) -> Option<PathBuf>;
    fn pick_module_package(&self) -> Option<PathBuf>;
    fn pick_source_inbox(&self, module_id: &ModuleId) -> Option<PathBuf>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeDialogPort;

impl DialogPort for NativeDialogPort {
    fn pick_workspace_root(&self) -> Option<PathBuf> {
        FileDialog::new()
            .set_title("Choose MyFitAnalytics workspace")
            .pick_folder()
    }

    fn pick_module_package(&self) -> Option<PathBuf> {
        FileDialog::new()
            .set_title("Install MyFitAnalytics module package")
            .add_filter(
                "MyFitAnalytics packages",
                &["mfasource", "mfadashboard", "mfalocale"],
            )
            .pick_file()
    }

    fn pick_source_inbox(&self, module_id: &ModuleId) -> Option<PathBuf> {
        FileDialog::new()
            .set_title(format!("Choose inbox for {}", module_id))
            .pick_folder()
    }
}
