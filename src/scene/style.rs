//! Properties: what scripts can read and set on an element, each kept where Freya keeps
//! it. Groups of them several kinds of element share have a module here; each kind of
//! element says which groups it has, and how to reach each in its Freya element.

/// A property with one value, for the tables in the group modules: `$field` of the
/// target, read with `$format` and set from what `$parse` makes of the value.
macro_rules! one {
    ($name:literal, $syntax:expr, $description:literal, |$target:ident| $field:expr, $parse:expr, $format:expr $(,)?) => {
        $crate::scene::style::Property {
            name: $name,
            syntax: $syntax,
            description: $description,
            access: $crate::scene::style::Access::One {
                get: |$target| ($format)(&$field),
                set: |$target, value| {
                    $field = ($parse)(value)?;
                    Ok(())
                },
            },
        }
    };
}

pub mod container;
pub mod decoration;
pub mod effects;
pub mod layout;
pub mod text;

use std::borrow::Cow;

/// How a property is read and changed.
pub enum Access<T: 'static> {
    /// A single value, which setting replaces.
    One {
        get: fn(&T) -> String,
        set: fn(&mut T, &str) -> Result<(), String>,
    },
    /// A list, which setting adds to, like Freya's setter for it.
    List {
        get: fn(&T) -> Vec<String>,
        add: fn(&mut T, &str) -> Result<(), String>,
        clear: fn(&mut T),
    },
}

/// A property scripts can read and set.
pub struct Property<T: 'static> {
    pub name: &'static str,
    /// The values it takes.
    pub syntax: &'static str,
    /// What it does, in a line.
    pub description: &'static str,
    pub access: Access<T>,
}

/// One of Freya's setters that sets other properties, like `expanded()`.
pub struct Shorthand<T: 'static> {
    pub name: &'static str,
    pub description: &'static str,
    pub apply: fn(&mut T),
}

/// Properties kept in a `T` of Freya's.
pub struct Group<T: 'static> {
    pub name: &'static str,
    /// What they're for.
    pub description: &'static str,
    pub properties: &'static [Property<T>],
    pub shorthands: &'static [Shorthand<T>],
}

/// A property's value, as text.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    One(String),
    List(Vec<String>),
}

/// One value as a string, a list as an array of them.
impl serde::Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::One(value) => serializer.serialize_str(value),
            Value::List(items) => serializer.collect_seq(items),
        }
    }
}

/// What setting or clearing a property can go wrong with.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PropertyError {
    /// The value didn't parse.
    #[error("{name}: {error}")]
    Invalid { name: &'static str, error: String },
    /// Only list properties can be cleared.
    #[error("{0} isn't a list, so it can't be cleared")]
    NotAList(&'static str),
}

/// A group of properties, reached through an element of type `E`.
pub trait Part<E> {
    /// Its value, if it has a property called `name`.
    fn get(&self, element: &E, name: &str) -> Option<Value>;
    /// Sets the property called `name`, if it has one: replaces it, or adds to a list.
    fn set(&self, element: &mut E, name: &str, value: &str) -> Option<Result<(), PropertyError>>;
    /// Empties the list property called `name`, if it has one.
    fn clear(&self, element: &mut E, name: &str) -> Option<Result<(), PropertyError>>;
    /// Applies the shorthand called `name`, if it has one.
    fn shorthand(&self, element: &mut E, name: &str) -> Option<()>;
    /// Every property's name and value.
    fn values(&self, element: &E) -> Vec<(&'static str, Value)>;
    /// The docs for its properties, with their values in `default`.
    fn docs(&self, default: &E) -> GroupDocs;
}

/// A group, reached through an element of type `E` that keeps its `T`.
pub struct Reach<E: 'static, T: Clone + 'static> {
    pub group: &'static Group<T>,
    /// The `T`, or a default one if the element has none yet.
    pub read: fn(&E) -> Cow<'_, T>,
    /// The `T`, made if the element has none yet.
    pub write: fn(&mut E) -> &mut T,
}

impl<E, T: Clone> Reach<E, T> {
    fn property(&self, name: &str) -> Option<&'static Property<T>> {
        self.group
            .properties
            .iter()
            .find(|property| property.name == name)
    }
}

impl<E, T: Clone> Part<E> for Reach<E, T> {
    fn get(&self, element: &E, name: &str) -> Option<Value> {
        let property = self.property(name)?;
        Some(read(property, &(self.read)(element)))
    }

    fn set(&self, element: &mut E, name: &str, value: &str) -> Option<Result<(), PropertyError>> {
        let property = self.property(name)?;
        let target = (self.write)(element);
        let result = match &property.access {
            Access::One { set, .. } => set(target, value),
            Access::List { add, .. } => add(target, value),
        };
        Some(result.map_err(|error| PropertyError::Invalid {
            name: property.name,
            error,
        }))
    }

    fn clear(&self, element: &mut E, name: &str) -> Option<Result<(), PropertyError>> {
        let property = self.property(name)?;
        Some(match &property.access {
            Access::List { clear, .. } => {
                clear((self.write)(element));
                Ok(())
            }
            Access::One { .. } => Err(PropertyError::NotAList(property.name)),
        })
    }

    fn shorthand(&self, element: &mut E, name: &str) -> Option<()> {
        let shorthand = self
            .group
            .shorthands
            .iter()
            .find(|shorthand| shorthand.name == name)?;
        (shorthand.apply)((self.write)(element));
        Some(())
    }

    fn values(&self, element: &E) -> Vec<(&'static str, Value)> {
        let target = (self.read)(element);
        self.group
            .properties
            .iter()
            .map(|property| (property.name, read(property, &target)))
            .collect()
    }

    fn docs(&self, default: &E) -> GroupDocs {
        let target = (self.read)(default);
        GroupDocs {
            name: self.group.name,
            description: self.group.description,
            properties: self
                .group
                .properties
                .iter()
                .map(|property| PropertyDocs {
                    name: property.name,
                    syntax: property.syntax,
                    description: property.description,
                    default: read(property, &target),
                    list: matches!(property.access, Access::List { .. }),
                })
                .collect(),
            shorthands: self
                .group
                .shorthands
                .iter()
                .map(|shorthand| (shorthand.name, shorthand.description))
                .collect(),
        }
    }
}

fn read<T>(property: &Property<T>, target: &T) -> Value {
    match &property.access {
        Access::One { get, .. } => Value::One(get(target)),
        Access::List { get, .. } => Value::List(get(target)),
    }
}

/// A group's properties and shorthands, for the docs.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupDocs {
    pub name: &'static str,
    pub description: &'static str,
    pub properties: Vec<PropertyDocs>,
    pub shorthands: Vec<(&'static str, &'static str)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDocs {
    pub name: &'static str,
    pub syntax: &'static str,
    pub description: &'static str,
    /// Its value on a new element.
    pub default: Value,
    /// Whether setting it adds to a list.
    pub list: bool,
}

/// The parts of an element of type `E`, all as one: finds which part has a property.
pub struct Parts<E: 'static>(pub &'static [&'static dyn Part<E>]);

impl<E> Parts<E> {
    pub fn get(&self, element: &E, name: &str) -> Option<Value> {
        self.0.iter().find_map(|part| part.get(element, name))
    }

    pub fn set(
        &self,
        element: &mut E,
        name: &str,
        value: &str,
    ) -> Option<Result<(), PropertyError>> {
        self.0
            .iter()
            .find_map(|part| part.set(element, name, value))
    }

    pub fn clear(&self, element: &mut E, name: &str) -> Option<Result<(), PropertyError>> {
        self.0.iter().find_map(|part| part.clear(element, name))
    }

    pub fn shorthand(&self, element: &mut E, name: &str) -> Option<()> {
        self.0.iter().find_map(|part| part.shorthand(element, name))
    }

    /// The properties whose values differ from those in `default`.
    pub fn changed(&self, element: &E, default: &E) -> Vec<(&'static str, Value)> {
        self.0
            .iter()
            .flat_map(|part| {
                let defaults = part.values(default);
                part.values(element)
                    .into_iter()
                    .zip(defaults)
                    .filter(|((_, value), (_, default))| value != default)
                    .map(|(changed, _)| changed)
            })
            .collect()
    }

    pub fn docs(&self, default: &E) -> Vec<GroupDocs> {
        self.0.iter().map(|part| part.docs(default)).collect()
    }
}

/// A property whose value is optional in Freya, where none means `none_shows`: shown
/// as that, and parsed as none from `none_word`.
pub fn optional<T>(value: Option<&T>, format: impl Fn(&T) -> String, none_shows: &str) -> String {
    value.map_or_else(|| none_shows.to_owned(), format)
}

/// Parses `input` as none if it's `none_word`, otherwise with `parse`.
pub fn parse_optional<T>(
    input: &str,
    none_word: &str,
    parse: impl Fn(&str) -> Result<T, String>,
) -> Result<Option<T>, String> {
    if input.trim() == none_word {
        Ok(None)
    } else {
        parse(input).map(Some)
    }
}

/// `items` shown with `format`, for a list property.
pub fn list<T>(items: &[T], format: impl Fn(&T) -> String) -> Vec<String> {
    items.iter().map(format).collect()
}

/// Text as a property value: trimmed, and unquoted if it's in quotes.
pub fn text_value(input: &str) -> Cow<'static, str> {
    let trimmed = input.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(trimmed);
    Cow::Owned(unquoted.to_owned())
}
