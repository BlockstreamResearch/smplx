//! Transaction inclusion verification using Merkle proofs for Liquid/Elements blocks.
//!
//! This module provides SPV (Simplified Payment Verification) functionality to prove
//! a transaction exists in a block without downloading all transactions.

use simplicityhl::elements::hashes::{Hash, HashEngine};
use simplicityhl::elements::{Block, TxMerkleNode, Txid};

/// Merkle proof: (`transaction_index`, `sibling_hashes`)
pub type MerkleProof = (usize, Vec<TxMerkleNode>);

/// Constructs a Merkle inclusion proof (Merkle branch).
///
/// For a transaction TXID in a block, using Bitcoin consensus Merkle tree construction rules
/// (pairwise double-SHA256 hashing with odd-hash duplication).
///
/// Liquid inherits the same Merkle tree semantics via the Elements codebase:
/// <https://developer.bitcoin.org/reference/block_chain.html>
///
/// Returns `None` if the transaction is not present in the block.
#[must_use]
pub fn merkle_branch(tx: &Txid, block: &Block) -> Option<MerkleProof> {
    if block.txdata.is_empty() {
        return None;
    }

    let tx_index = block.txdata.iter().position(|t| &t.txid() == tx)?;

    Some((tx_index, build_merkle_branch(tx_index, block)))
}

/// Verifies a Merkle inclusion proof (Merkle branch).
///
/// For a transaction TXID against the given Merkle root using Bitcoin consensus Merkle tree rules
/// (pairwise double-SHA256 hashing with left/right ordering).
///
/// Liquid inherits the same Merkle tree semantics via the Elements codebase:
/// <https://developer.bitcoin.org/reference/block_chain.html>
///
/// Returns `true` if the proof commits the transaction to the given root.
#[must_use]
pub fn verify_tx(tx: &Txid, root: &TxMerkleNode, proof: &MerkleProof) -> bool {
    root.eq(&compute_merkle_root_from_branch(tx, proof.0, &proof.1))
}

fn build_merkle_branch(tx_index: usize, block: &Block) -> Vec<TxMerkleNode> {
    if block.txdata.is_empty() || block.txdata.len() == 1 {
        return vec![];
    }

    let mut branch = vec![];
    let mut layer = block
        .txdata
        .iter()
        .map(|tx| TxMerkleNode::from_raw_hash(*tx.txid().as_raw_hash()))
        .collect::<Vec<_>>();
    let mut index = tx_index;

    // Bottom-up traversal: pair nodes, hash parents, collect siblings along path to root
    while layer.len() > 1 {
        let mut next_layer = vec![];

        for i in (0..layer.len()).step_by(2) {
            let left = layer[i];
            let right = if i + 1 < layer.len() { layer[i + 1] } else { layer[i] };

            let mut eng = TxMerkleNode::engine();
            eng.input(left.as_raw_hash().as_byte_array());
            eng.input(right.as_raw_hash().as_byte_array());

            next_layer.push(TxMerkleNode::from_engine(eng));

            if index / 2 == i / 2 {
                let sibling = if index.is_multiple_of(2) { right } else { left };
                branch.push(sibling);
            }
        }

        index /= 2;
        layer = next_layer;
    }

    branch
}

fn compute_merkle_root_from_branch(tx: &Txid, tx_index: usize, branch: &[TxMerkleNode]) -> TxMerkleNode {
    let mut res = TxMerkleNode::from_raw_hash(*tx.as_raw_hash());
    let mut pos = tx_index;

    for leaf in branch {
        let mut eng = TxMerkleNode::engine();

        if pos & 1 == 0 {
            eng.input(res.as_raw_hash().as_byte_array());
            eng.input(leaf.as_raw_hash().as_byte_array());
        } else {
            eng.input(leaf.as_raw_hash().as_byte_array());
            eng.input(res.as_raw_hash().as_byte_array());
        }
        res = TxMerkleNode::from_engine(eng);

        pos >>= 1;
    }

    res
}

#[cfg(test)]
mod test {

    use super::*;

    /// Taken from rust-elements
    /// <https://github.com/ElementsProject/rust-elements/blob/master/src/internal_macros.rs>
    macro_rules! hex_deserialize(
        ($e:expr) => ({
            use simplicityhl::elements::encode::deserialize;

            fn hex_char(c: char) -> u8 {
                match c {
                    '0' => 0,
                    '1' => 1,
                    '2' => 2,
                    '3' => 3,
                    '4' => 4,
                    '5' => 5,
                    '6' => 6,
                    '7' => 7,
                    '8' => 8,
                    '9' => 9,
                    'a' | 'A' => 10,
                    'b' | 'B' => 11,
                    'c' | 'C' => 12,
                    'd' | 'D' => 13,
                    'e' | 'E' => 14,
                    'f' | 'F' => 15,
                    x => panic!("Invalid character {} in hex string", x),
                }
            }

            let mut ret = Vec::with_capacity($e.len() / 2);
            let mut byte = 0;
            for (ch, store) in $e.chars().zip([false, true].iter().cycle()) {
                byte = (byte << 4) + hex_char(ch);
                if *store {
                    ret.push(byte);
                    byte = 0;
                }
            }
            deserialize(&ret).expect("deserialize object")
        });
    );

    // Unfortunately, `hex_deserialize` macro aforehead returns error trying deserialize
    // blocks from elements-cli regtest, so this block, taken from `elements::Block::block`, is
    // the only test case I have found so far.
    const BLOCK_STR: &str = include_str!("./assets/test-tx-incl-block.hex");

    #[test]
    fn test_merkle_branch_construction() {
        let block: Block = hex_deserialize!(BLOCK_STR);

        assert_eq!(block.txdata.len(), 3);

        let tx = block.txdata[1].txid();
        let proof = merkle_branch(&tx, &block).expect("Failed to find tx in block");

        assert!(
            verify_tx(&tx, &block.header.merkle_root, &proof),
            "Invalid merkle proof"
        );
    }
}
