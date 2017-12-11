use std::sync::Arc;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::ops::{Add, Sub, Div, Mul};

use super::*;

use ident::*;

pub type Nil = ();
pub type Int = i32;
pub type Str = Arc<str>;
pub type List = Arc<RefCell<VecDeque<Value>>>;
pub type Record = Arc<RefCell<HashMap<Ident, Value>>>;

pub trait Extract: Sized {
    const TYPE_NAME: &'static str;

    fn extract(value: Value) -> Result<Self>;
}

macro_rules! impl_value {
    ( $( $type:ident ),* ) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum Value {
            $( $type($type), )*
        }

        impl Value {
            pub fn type_name(&self) -> &'static str {
                match *self {
                    $( Value::$type(_) => stringify!($type), )*
                }
            }
        }

        $(
            impl From<$type> for Value {
                fn from(t: $type) -> Self {
                    Value::$type(t)
                }
            }

            impl Extract for $type {
                const TYPE_NAME: &'static str = stringify!($type);

                fn extract(value: Value) -> Result<Self> {
                    match value {
                        Value::$type(ok) => Ok(ok),

                        other => Err(Error::TypeMismatch {
                            expected: Self::TYPE_NAME,
                            found: other.type_name(),
                        }),
                    }
                }
            }
        )*
    }
}

impl_value!(Nil, Int, Str, List, Record, Ident);

impl Value {
    pub fn from_slice<T: AsRef<[Value]>>(slice: T) -> Self {
        let slice = slice.as_ref();
        let vec_deque = slice.iter().cloned().collect();
        let list = Arc::new(RefCell::new(vec_deque));
        Value::List(list)
    }

    pub fn index(self, rhs: Self) -> Result<Self> {
        match self {
            Value::List(lhs) => {
                let lhs = lhs.borrow();
                let rhs = Int::extract(rhs)?;

                if rhs < 0 {
                    return Err(Error::NegativeIndex);
                }

                let rhs = rhs as usize;

                lhs.get(rhs).cloned().ok_or(Error::IndexOutOfBounds)
            },

            Value::Record(lhs) => {
                let lhs = lhs.borrow();
                let rhs = Ident::extract(rhs)?;
                lhs.get(&rhs).cloned().ok_or(Error::IndexOutOfBounds)
            },

            other => Err(Error::TypeMismatch {
                expected: "List|Record",
                found: other.type_name(),
            }),
        }
    }

    pub fn insert(self, key: Self, val: Self) -> Result<()> {
        match self {
            Value::List(lhs) => {
                let mut lhs = lhs.borrow_mut();
                let key = Int::extract(key)?;
                if key < 0 {
                    return Err(Error::NegativeIndex);
                }
                let rhs = key as usize;

                if rhs > lhs.len() {
                    return Err(Error::IndexOutOfBounds);
                }

                lhs[rhs] = val;
                Ok(())
            },

            Value::Record(lhs) => {
                let mut lhs = lhs.borrow_mut();
                let key = Ident::extract(key)?;
                *lhs.entry(key).or_insert(().into()) = val;
                Ok(())
            },

            other => Err(Error::TypeMismatch {
                expected: "List|Record",
                found: other.type_name(),
            }),
        }
    }
}

impl Extract for Value {
    const TYPE_NAME: &'static str = "Anything";

    fn extract(value: Self) -> Result<Self> {
        Ok(value)
    }
}

impl Add for Value {
    type Output = Result<Self>;

    fn add(self, rhs: Self) -> Result<Self> {
        match self {
            Value::Int(lhs) => {
                let rhs = Int::extract(rhs)?;
                Ok((lhs + rhs).into())
            },

            Value::List(lhs) => match rhs {
                Value::List(rhs) => {
                    let lhs = lhs.borrow();
                    let rhs = rhs.borrow();

                    let list: VecDeque<Value> = lhs.iter().cloned().chain({
                        rhs.iter().cloned()
                    }).collect();

                    Ok(Value::List(Arc::new(RefCell::new(list))))
                },

                other => {
                    let lhs = lhs.borrow();
                    let mut list = VecDeque::with_capacity(lhs.len() + 1);
                    list.push_back(other);
                    Ok(Value::List(Arc::new(RefCell::new(list))))
                },
            },

            Value::Str(lhs) => {
                Ok(Str::from(format!("{}{}", lhs, rhs)).into())
            },

            _ => Err(Error::IllegalAdd),
        }
    }
}

impl Sub for Value {
    type Output = Result<Self>;

    fn sub(self, rhs: Self) -> Result<Self> {
        let lhs = Int::extract(self)?;
        let rhs = Int::extract(rhs)?;
        Ok((lhs - rhs).into())
    }
}

impl Div for Value {
    type Output = Result<Self>;

    fn div(self, rhs: Self) -> Result<Self> {
        match (self, rhs) {
            (_, Value::Int(0)) => {
                Err(Error::DividedByZero)
            },

            (Value::Int(lhs), Value::Int(rhs)) => {
                Ok((lhs / rhs).into())
            },

            (_, other) => {
                Err(Error::TypeMismatch {
                    expected: "Int",
                    found: other.type_name(),
                })
            },
        }
    }
}

impl Mul for Value {
    type Output = Result<Self>;

    fn mul(self, rhs: Self) -> Result<Self> {
        let rhs = Int::extract(rhs)?;

        match self {
            Value::Int(lhs) => {
                Ok((lhs * rhs).into())
            },

            Value::Str(lhs) => {
                if rhs < 0 {
                    return Err(Error::NegativeRepetition);
                }

                let rhs = rhs as usize;

                let mut buf = String::with_capacity(lhs.len() * rhs);

                for _ in 0 .. rhs {
                    buf.push_str(&lhs);
                }

                Ok((Str::from(buf)).into())
            },

            _ => {
                Err(Error::IllegalMultiply)
            },
        }
    }
}

use std::fmt::{self, Display};

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Value::Nil(_) => write!(f, "nil"),
            Value::Int(i) => write!(f, "{}", i),
            Value::Str(ref s) => write!(f, "{}", s),
            Value::Ident(ref id) => write!(f, "{}", id),

            Value::List(ref l) => {
                let contents = l.borrow().iter().map(|item| {
                    format!("{}", item)
                }).collect::<Vec<String>>().join(", ");

                write!(f, "[{}]", contents)
            },

            Value::Record(ref rec) => {
                let contents = rec.borrow().iter().map(|(k, v)| {
                    format!("{}: {}", k, v)
                }).collect::<Vec<_>>().join(", ");

                write!(f, "{{ {} }}", contents)
            },
        }
    }
}
