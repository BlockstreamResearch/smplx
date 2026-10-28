/// A conversion failure from any fallible alias type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AliasError {
    /// A field element conversion failed.
    #[error(transparent)]
    Fe(#[from] FeConversionError),
    /// A finite point conversion failed.
    #[error(transparent)]
    Point(#[from] PointConversionError),
    /// An affine point conversion failed.
    #[error(transparent)]
    Ge(#[from] GeConversionError),
    /// A Jacobian point conversion failed.
    #[error(transparent)]
    Gej(#[from] GejConversionError),
    /// A SHA256 context conversion failed.
    #[error(transparent)]
    Ctx8(#[from] Ctx8ConversionError),
    /// A block height conversion failed.
    #[error(transparent)]
    Height(#[from] HeightConversionError),
    /// A timestamp conversion failed.
    #[error(transparent)]
    Time(#[from] TimeConversionError),
    /// A signature conversion failed.
    #[error(transparent)]
    Signature(#[from] SignatureConversionError),
    /// A scalar conversion failed.
    #[error(transparent)]
    Scalar(#[from] ScalarConversionError),
    /// An x-only public key conversion failed.
    #[error(transparent)]
    Pubkey(#[from] PubkeyConversionError),
    /// An asset conversion failed.
    #[error(transparent)]
    Asset1(#[from] Asset1ConversionError),
    /// An amount conversion failed.
    #[error(transparent)]
    Amount1(#[from] Amount1ConversionError),
    /// A nonce conversion failed.
    #[error(transparent)]
    Nonce(#[from] NonceConversionError),
    /// A reissuance token amount conversion failed.
    #[error(transparent)]
    TokenAmount1(#[from] TokenAmount1ConversionError),
}

/// Failed conversion of a canonical field element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FeConversionError {
    /// A canonical field element must be below the secp256k1 field prime.
    #[error("field element is not below the secp256k1 field prime")]
    InvalidFieldElement,
}

/// Failed conversion of a finite secp256k1 point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PointConversionError {
    /// Coordinates or a compressed encoding must describe a finite secp256k1 point.
    #[error("invalid finite secp256k1 point or compressed encoding")]
    InvalidCurvePoint,
}

/// Failed conversion of affine secp256k1 coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GeConversionError {
    /// A coordinate is not a canonical field element.
    #[error(transparent)]
    InvalidFieldElement(#[from] FeConversionError),
    /// The coordinates or source point do not describe a finite curve point.
    #[error(transparent)]
    InvalidCurvePoint(#[from] PointConversionError),
}

/// Failed conversion of a finite Jacobian secp256k1 point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GejConversionError {
    /// The affine point used to construct the Jacobian point is invalid.
    #[error(transparent)]
    InvalidAffinePoint(#[from] GeConversionError),
}

/// Failed construction of an unfinalized SHA256 context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Ctx8ConversionError {
    /// A SHA256 context must have fewer than 64 buffered bytes.
    #[error("SHA256 context buffer length {0} must be less than 64")]
    ContextBufferLength(usize),
    /// The pending buffer must match the total byte count modulo 64.
    #[error("SHA256 context buffer length does not match total byte count")]
    ContextLengthMismatch,
    /// SHA256 supports fewer than 2^61 input bytes.
    #[error("SHA256 context byte count must be less than 2^61")]
    ContextLengthOverflow,
}

/// Failed conversion of an absolute block height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HeightConversionError {
    /// Absolute block heights must be below the timestamp threshold.
    #[error("block height must be below 500000000")]
    InvalidHeight,
}

/// Failed conversion of an absolute timestamp or jet time fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TimeConversionError {
    /// A jet time is zero or an absolute timestamp at or above the threshold.
    #[error("time must be zero or at least 500000000")]
    InvalidTime,
}

/// Failed conversion of a serialized BIP340 signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SignatureConversionError {
    /// A serialized BIP340 signature must contain exactly 64 bytes.
    #[error("BIP340 signature must contain exactly 64 bytes")]
    InvalidSignature,
}

/// Failed conversion of a canonical secp256k1 scalar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ScalarConversionError {
    /// A scalar must be below the secp256k1 curve order.
    #[error("scalar is not below the secp256k1 curve order")]
    InvalidScalar,
}

/// Failed conversion of an x-only secp256k1 public key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PubkeyConversionError {
    /// An x-only public key must represent a valid secp256k1 point.
    #[error("invalid x-only secp256k1 public key")]
    InvalidPubkey,
    /// The source point is not a valid finite curve point.
    #[error(transparent)]
    InvalidCurvePoint(#[from] PointConversionError),
}

/// Failed conversion of a non-null Elements asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Asset1ConversionError {
    /// An absent asset must be represented separately with an option.
    #[error("Asset1 cannot represent a null transaction field")]
    NullField,
    /// The selector and coordinate do not form a valid asset generator.
    #[error("invalid confidential commitment or compressed point encoding")]
    InvalidConfidentialEncoding,
}

/// Failed conversion of a non-null Elements amount.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Amount1ConversionError {
    /// An absent amount must be represented separately with an option.
    #[error("Amount1 cannot represent a null transaction field")]
    NullField,
    /// The selector and coordinate do not form a valid Pedersen commitment.
    #[error("invalid confidential commitment or compressed point encoding")]
    InvalidConfidentialEncoding,
}

/// Failed conversion of a non-null Elements nonce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NonceConversionError {
    /// An absent nonce must be represented separately with an option.
    #[error("Nonce cannot represent a null transaction field")]
    NullField,
    /// The source point is not a valid finite curve point.
    #[error(transparent)]
    InvalidCurvePoint(#[from] PointConversionError),
}

/// Failed conversion of a non-null reissuance-token amount.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TokenAmount1ConversionError {
    /// An absent token amount must be represented separately with an option.
    #[error("TokenAmount1 cannot represent a null transaction field")]
    NullField,
    /// The selector and coordinate do not form a valid Pedersen commitment.
    #[error("invalid confidential commitment or compressed point encoding")]
    InvalidConfidentialEncoding,
}
