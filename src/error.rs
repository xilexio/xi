use screeps::OutOfBoundsError;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone)]
pub struct GenericError {
    pub description: String,
}

impl Display for GenericError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl Error for GenericError {}

impl From<OutOfBoundsError> for GenericError {
    fn from(value: OutOfBoundsError) -> Self {
        GenericError {
            description: format!("{}", value),
        }
    }
}

// impl<T> Display for GenericError {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "{}", self.description)
//     }
// }

// impl From<Result<T>, OutOfBoundsError> for GenericError {
//     fn from(error: OutOfBoundsError) -> Self {
//         GenericError {
//             description: format!("{}", error),
//         }
//     }
// }

// impl<E> From<E> for Box<dyn Error>
// where
//     E: Error
// {
//     fn from(err: E) -> Box<dyn Error> {
//         Box::new(err)
//     }
// }
