use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct Scripts {
    pub preinstall: Option<String>,
    pub install: Option<String>,
    pub postinstall: Option<String>,
    pub prepare: Option<String>,
    pub preprepare: Option<String>,
    pub postprepare: Option<String>,
    pub prepublish: Option<String>,
}

impl Scripts {
    pub fn has_any_script(&self) -> bool {
        self.preinstall.is_some() || self.install.is_some() || self.postinstall.is_some()
    }

    pub fn get_script(&self, script_type: &str) -> Option<&String> {
        match script_type {
            "preinstall" => self.preinstall.as_ref(),
            "install" => self.install.as_ref(),
            "postinstall" => self.postinstall.as_ref(),
            "prepare" => self.prepare.as_ref(),
            "preprepare" => self.preprepare.as_ref(),
            "postprepare" => self.postprepare.as_ref(),
            "prepublish" => self.prepublish.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub path: PathBuf,
    pub bin_files: Vec<(String, String)>, // (bin_name, relative_path)
    pub scripts: Scripts,
    #[allow(dead_code)]
    pub scope: Option<String>, // exp "@babel"
    pub fullname: String, // exp "@babel/parser"
    #[allow(dead_code)]
    pub name: String, // exp parser
    pub version: String,
}

impl PackageInfo {
    pub fn get_bin_dir(&self) -> Option<PathBuf> {
        self.path
            .ancestors()
            .find(|p| p.ends_with("node_modules"))
            .map(|p| p.to_path_buf().join(".bin"))
    }

    #[allow(dead_code)]
    pub fn has_bin_files(&self) -> bool {
        !self.bin_files.is_empty()
    }

    #[allow(dead_code)]
    pub fn needs_processing(&self) -> bool {
        self.has_bin_files() || self.scripts.has_any_script()
    }

    pub fn has_script(&self) -> bool {
        self.scripts.has_any_script()
    }
}
