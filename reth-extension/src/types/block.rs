use bytes::Bytes;
use consensus_config::DIGEST_LENGTH;
use consensus_core::Transaction;
use fastcrypto::hash::{Digest, HashFunction};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    hash::{Hash, Hasher},
};

pub type Block = Vec<Transaction>;
/// A Block with its signature, before they are verified.
///
/// Note: `BlockDigest` is computed over this struct, so any added field (without `#[serde(skip)]`)
/// will affect the values of `BlockDigest` and `BlockRef`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct SignedBlock {
    inner: Block,
    signature: Bytes,
}

impl SignedBlock {
    /// Should only be used when constructing the genesis blocks
    pub(crate) fn new_genesis(block: Block) -> Self {
        Self {
            inner: block,
            signature: Bytes::default(),
        }
    }

    // pub(crate) fn new(block: Block, protocol_keypair: &ProtocolKeyPair) -> ConsensusResult<Self> {
    //     let signature = compute_block_signature(&block, protocol_keypair)?;
    //     Ok(Self {
    //         inner: block,
    //         signature: Bytes::copy_from_slice(signature.to_bytes()),
    //     })
    // }

    pub(crate) fn signature(&self) -> &Bytes {
        &self.signature
    }

    // /// This method only verifies this block's signature. Verification of the full block
    // /// should be done via BlockVerifier.
    // pub(crate) fn verify_signature(&self, context: &Context) -> ConsensusResult<()> {
    //     let block = &self.inner;
    //     let committee = &context.committee;
    //     ensure!(
    //         committee.is_valid_index(block.author()),
    //         ConsensusError::InvalidAuthorityIndex {
    //             index: block.author(),
    //             max: committee.size() - 1
    //         }
    //     );
    //     let authority = committee.authority(block.author());
    //     verify_block_signature(block, self.signature(), &authority.protocol_key)
    // }

    /// Serialises the block using the bcs serializer
    // pub(crate) fn serialize(&self) -> Result<Bytes, bcs::Error> {
    //     let bytes = bcs::to_bytes(self)?;
    //     Ok(bytes.into())
    // }

    /// Clears signature for testing.
    #[cfg(test)]
    pub(crate) fn clear_signature(&mut self) {
        self.signature = Bytes::default();
    }
}

/// Digest of a `VerifiedBlock` or verified `SignedBlock`, which covers the `Block` and its
/// signature.
///
/// Note: the signature algorithm is assumed to be non-malleable, so it is impossible for another
/// party to create an altered but valid signature, producing an equivocating `BlockDigest`.
#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockDigest(pub [u8; DIGEST_LENGTH]);

impl BlockDigest {
    /// Lexicographic min & max digest.
    pub const MIN: Self = Self([u8::MIN; DIGEST_LENGTH]);
    pub const MAX: Self = Self([u8::MAX; DIGEST_LENGTH]);
}

impl Hash for BlockDigest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.0[..8]);
    }
}

impl From<BlockDigest> for Digest<{ DIGEST_LENGTH }> {
    fn from(hd: BlockDigest) -> Self {
        Digest::new(hd.0)
    }
}

impl fmt::Display for BlockDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.0)
                .get(0..4)
                .ok_or(fmt::Error)?
        )
    }
}

impl fmt::Debug for BlockDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.0)
        )
    }
}

impl AsRef<[u8]> for BlockDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
