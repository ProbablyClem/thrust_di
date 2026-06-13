pub struct ComponentInfo {
    pub name: String,
    pub deps: Vec<(String, String)>, // (field_name, type_name)
}

pub struct BeanInfo {
    pub name: String,
    pub fn_name: String,
    pub is_async: bool,
    pub deps: Vec<String>, // dep type names (unwrapped Arc<T> from params)
    pub module_path: String,
}

pub struct LayerInfo {
    pub fn_name: String,
    pub module_path: String,
}

pub struct TraitImpl {
    pub trait_name: String, // the implemented trait, e.g. `UserRepository`
    pub concrete: String,   // the implementing type, e.g. `PostgresUserRepository`
}

pub struct RawRouteInfo {
    pub fn_name: String,
    pub method: String,
    pub path: String,
    pub module_path: String,
    pub arc_params: Vec<String>,
}

pub struct RouteInfo {
    pub fn_name: String,
    pub method: String,
    pub path: String,
    pub module_path: String,
    pub service_params: Vec<String>,
}

pub const HTTP_METHODS: &[&str] = &["get", "post", "put", "delete", "patch"];
