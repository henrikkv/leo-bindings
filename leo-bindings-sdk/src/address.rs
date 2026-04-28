use leo_bindings_core::{FromValue, ToValue};
use snarkvm::prelude::{Address as SvmAddress, Literal, Network, Plaintext, ProgramID, Value};
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Address<N: Network>(pub SvmAddress<N>);

impl<N: Network> TryFrom<&str> for Address<N> {
    type Error = crate::Error;

    fn try_from(s: &str) -> crate::Result<Self> {
        if let Ok(addr) = SvmAddress::<N>::from_str(s) {
            return Ok(Self(addr));
        }
        Ok(Self(ProgramID::<N>::from_str(s)?.to_address()?))
    }
}

impl<N: Network> TryFrom<String> for Address<N> {
    type Error = crate::Error;

    fn try_from(s: String) -> crate::Result<Self> {
        Self::try_from(s.as_str())
    }
}

impl<N: Network> TryFrom<ProgramID<N>> for Address<N> {
    type Error = crate::Error;

    fn try_from(program_id: ProgramID<N>) -> crate::Result<Self> {
        Ok(Self(program_id.to_address()?))
    }
}

impl<N: Network> TryFrom<&ProgramID<N>> for Address<N> {
    type Error = crate::Error;

    fn try_from(program_id: &ProgramID<N>) -> crate::Result<Self> {
        Ok(Self(program_id.to_address()?))
    }
}

impl<N: Network> From<SvmAddress<N>> for Address<N> {
    fn from(a: SvmAddress<N>) -> Self {
        Self(a)
    }
}

impl<N: Network> From<Address<N>> for SvmAddress<N> {
    fn from(a: Address<N>) -> Self {
        a.0
    }
}

impl<N: Network> Deref for Address<N> {
    type Target = SvmAddress<N>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<N: Network> fmt::Debug for Address<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl<N: Network> fmt::Display for Address<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<N: Network> ToValue<N> for Address<N> {
    fn to_value(&self) -> Value<N> {
        Value::from(Literal::Address(self.0))
    }
}

impl<N: Network> FromValue<N> for Address<N> {
    fn from_value(value: Value<N>) -> Self {
        match value {
            Value::Plaintext(Plaintext::Literal(Literal::Address(v), _)) => Self(v),
            _ => panic!("Expected address type."),
        }
    }
}
