struct SceneBase {
    name: String,
    // Array of functions named 'updateFunctions'
}

impl SceneBase {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}
