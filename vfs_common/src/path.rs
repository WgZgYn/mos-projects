/// `/` `.` `..`
pub enum Component<'a> {
    Root,
    Current,
    Parent,
    Normal(&'a str),
}

pub trait AsPath {
    fn as_path(&'_ self) -> impl Iterator<Item = Component<'_>>;
}

impl<T: AsRef<str>> AsPath for T {
    fn as_path(&'_ self) -> impl Iterator<Item = Component<'_>> {
        str_as_path(self.as_ref())
    }
}

pub fn str_as_path(path: &str) -> impl Iterator<Item = Component<'_>>  {
    let path = path.trim();
    let mut start_with_root = None;

    // this means this is an absolute path
    if path.starts_with('/') {
        start_with_root = Some(Component::Root);
    }

    // this part will never get '/' part
    start_with_root
        .into_iter()
        .chain(path.split('/').filter(|s| !s.is_empty()).map(|s| match s {
            "." => Component::Current,
            ".." => Component::Parent,
            _ => Component::Normal(s),
        }))
}
