//! Checked construction for fuzz strategies and callers with raw inputs.
//! Reexported key, scalar and signature types already expose their library
//! constructors. Byte payloads and u16/u64 quantities have no extra restrictions.

use simplicityhl::elements::{
    hashes::{HashEngine, sha256},
    locktime,
    secp256k1_zkp::constants,
};

use super::{
    AliasError, Confidential1, Ctx8, Fe, Ge, Gej, Generator, Height, PedersenCommitment, Point, Time, commitment,
};

impl Fe {
    /// secp256k1's field prime p = 2^256 - 2^32 - 977, in big-endian order.
    pub const MODULUS: [u8; 32] = constants::FIELD_SIZE;

    /// The canonical zero field element.
    pub const ZERO: Self = Self(constants::ZERO);

    /// The canonical multiplicative identity.
    pub const ONE: Self = Self(constants::ONE);

    /// Construct a canonical field element without modular reduction.
    ///
    /// # Errors
    /// Rejects values greater than or equal to [`Fe::MODULUS`]. Zero is valid.
    pub fn from_be_bytes(bytes: [u8; 32]) -> Result<Self, AliasError> {
        if bytes >= Self::MODULUS {
            return Err(AliasError::InvalidFieldElement);
        }
        Ok(Self(bytes))
    }
}

impl Ge {
    /// Construct a finite affine point using secp256k1's uncompressed-point parser.
    ///
    /// # Errors
    /// Rejects noncanonical coordinates or a point outside the curve.
    pub fn from_coordinates(x: Fe, y: Fe) -> Result<Self, AliasError> {
        Fe::from_be_bytes(x.0)?;
        Fe::from_be_bytes(y.0)?;
        let mut bytes = [0; 65];
        bytes[0] = 4;
        bytes[1..33].copy_from_slice(&x.0);
        bytes[33..].copy_from_slice(&y.0);
        Point::from_slice(&bytes).map_err(|_| AliasError::InvalidCurvePoint)?;
        Ok(Self { x, y })
    }
}

impl Gej {
    /// Construct a valid finite Jacobian point from affine coordinates, with Z = 1.
    ///
    /// # Errors
    /// Rejects noncanonical or off-curve affine coordinates, including (0, 0).
    pub fn from_affine(ge: Ge) -> Result<Self, AliasError> {
        Ok(Self {
            ge: Ge::from_coordinates(ge.x, ge.y)?,
            z: Fe::ONE,
        })
    }

    /// Construct the point at infinity, represented by Z = 0.
    #[must_use]
    pub const fn infinity() -> Self {
        Self {
            ge: Ge {
                x: Fe::ZERO,
                y: Fe::ZERO,
            },
            z: Fe::ZERO,
        }
    }
}

impl From<Point> for Gej {
    fn from(point: Point) -> Self {
        Self {
            ge: Ge::from(point),
            z: Fe::ONE,
        }
    }
}

impl Confidential1 {
    /// Validate a serialized generator, Pedersen commitment or compressed public key.
    ///
    /// # Errors
    /// Rejects unsupported prefixes, invalid coordinates and points off the curve.
    pub fn from_serialized(bytes: [u8; 33]) -> Result<Self, AliasError> {
        let valid = match bytes[0] {
            0x0a | 0x0b => Generator::from_slice(&bytes).is_ok(),
            0x08 | 0x09 => PedersenCommitment::from_slice(&bytes).is_ok(),
            0x02 | 0x03 => Point::from_slice(&bytes).is_ok(),
            _ => false,
        };
        if !valid {
            return Err(AliasError::InvalidConfidentialEncoding);
        }
        Ok(commitment(bytes))
    }
}

impl Ctx8 {
    /// Compute the unfinalized SHA256 context after consuming the given bytes.
    ///
    /// # Errors
    /// Rejects inputs of 2^61 bytes or more, which overflow SHA256's bit counter.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AliasError> {
        let length = u64::try_from(bytes.len()).map_err(|_| AliasError::ContextLengthOverflow)?;
        if length >= 1 << 61 {
            return Err(AliasError::ContextLengthOverflow);
        }
        let complete_len = bytes.len() - bytes.len() % 64;
        let mut engine = sha256::HashEngine::new();
        engine.input(&bytes[..complete_len]);
        let midstate = engine
            .midstate()
            .map_err(|_| AliasError::ContextLengthMismatch)?
            .to_parts()
            .0;
        Self::from_parts(bytes[complete_len..].to_vec(), length, midstate)
    }

    /// Construct a jet context from pending bytes, compression count and midstate.
    /// The state is accepted as supplied; its provenance cannot be verified.
    ///
    /// # Errors
    /// Rejects buffers of 64 or more bytes or a total byte count at or above 2^61.
    pub fn from_compressed_parts(
        buffer: Vec<u8>,
        compressed_blocks: u64,
        midstate: [u8; 32],
    ) -> Result<Self, AliasError> {
        if buffer.len() >= 64 {
            return Err(AliasError::ContextBufferLength(buffer.len()));
        }
        let length = compressed_blocks
            .checked_mul(64)
            .and_then(|n| n.checked_add(buffer.len() as u64))
            .ok_or(AliasError::ContextLengthOverflow)?;
        Self::from_parts(buffer, length, midstate)
    }
}

impl Height {
    /// Construct an absolute block height below the transaction timestamp threshold.
    ///
    /// # Errors
    /// Rejects values at or above 500,000,000. Raw construction remains available.
    pub fn from_consensus(height: u32) -> Result<Self, AliasError> {
        locktime::Height::from_consensus(height).map_err(|_| AliasError::InvalidHeight)?;
        Ok(Self(height))
    }
}

impl Time {
    /// Construct an absolute timestamp or the zero fallback returned by time jets.
    ///
    /// # Errors
    /// Rejects nonzero values below 500,000,000. Raw construction remains available.
    pub fn from_consensus(time: u32) -> Result<Self, AliasError> {
        if time != 0 {
            locktime::Time::from_consensus(time).map_err(|_| AliasError::InvalidTime)?;
        }
        Ok(Self(time))
    }
}
