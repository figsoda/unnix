use derive_more::From;
use knus::Decode;
use url::Url;

#[derive(Decode)]
pub struct Manifest {
    #[knus(child, default)]
    pub packages: Packages,
    #[knus(child, default = Caches { default: true, inner: Vec::new() })]
    pub caches: Caches,
    #[knus(child, default)]
    pub env: Env,
    #[knus(children)]
    pub sources: Vec<Source>,
}

#[derive(Decode, Default)]
pub struct Packages {
    #[knus(children)]
    pub inner: Vec<Package>,
}

#[derive(Decode)]
pub struct Package {
    #[knus(node_name)]
    pub name: String,
    #[knus(property)]
    pub attribute: Option<String>,
    #[knus(property)]
    pub outputs: Option<String>,
    #[knus(property, default = "default".into())]
    pub source: String,
}

#[derive(Decode)]
pub struct Caches {
    #[knus(property, default = true)]
    pub default: bool,
    #[knus(children)]
    pub inner: Vec<Cache>,
}

#[derive(Decode)]
pub struct Cache {
    #[knus(node_name)]
    pub url: Url,
}

#[derive(Decode, Default)]
pub struct Env {
    #[knus(children)]
    pub inner: Vec<Var>,
}

#[derive(Decode)]
pub struct Var {
    #[knus(node_name)]
    pub name: String,
    #[knus(argument)]
    pub value: String,
}

#[derive(Decode)]
pub enum Source {
    Hydra(Hydra),
}

#[derive(Decode)]
pub struct Hydra {
    #[knus(argument)]
    pub name: String,
    #[knus(child)]
    pub base: StringArgument,
    #[knus(child)]
    pub project: StringArgument,
    #[knus(child)]
    pub jobset: StringArgument,
    #[knus(child, default = "{attribute}.{system}".into())]
    pub job: StringArgument,
}

#[derive(Decode, From)]
#[from(forward)]
pub struct StringArgument {
    #[knus(argument)]
    pub inner: String,
}
