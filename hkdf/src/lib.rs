//! An implementation of HKDF, the [HMAC-based Extract-and-Expand Key Derivation Function][1].
//!
//! # Usage
//!
//! The most common way to use HKDF is as follows: you provide the Initial Key
//! Material (IKM) and an optional salt, then you expand it (perhaps multiple times)
//! into some Output Key Material (OKM) bound to an "info" context string.
//!
//! ```rust
//! use sha2::Sha256;
//! use hkdf::Hkdf;
//! use hmac::Hmac;
//! use hex_literal::hex;
//!
//! let ikm = hex!("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
//! let salt = hex!("000102030405060708090a0b0c");
//! let info = hex!("f0f1f2f3f4f5f6f7f8f9");
//!
//! let hk = Hkdf::<Hmac<Sha256>>::new(Some(&salt[..]), &ikm);
//! let mut okm = [0u8; 42];
//! hk.expand(&info, &mut okm)
//!     .expect("42 is a valid length for Sha256 to output");
//!
//! let expected = hex!("
//!     3cb25f25faacd57a90434f64d0362f2a
//!     2d2d0a90cf1a5a4c5db02d56ecc4c5bf
//!     34007208d5b887185865
//! ");
//! assert_eq!(okm[..], expected[..]);
//! ```
//!
//! Normally the PRK (Pseudo-Random Key) remains hidden within the HKDF
//! object, but if you need to access it, use [`Hkdf::extract`] instead of
//! [`Hkdf::new`].
//!
//! ```rust
//! # use sha2::Sha256;
//! # use hkdf::Hkdf;
//! # use hex_literal::hex;
//! # use hmac::Hmac;
//! # let ikm = hex!("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
//! # let salt = hex!("000102030405060708090a0b0c");
//!
//! let (prk, hk) = Hkdf::<Hmac<Sha256>>::extract(Some(&salt[..]), &ikm);
//! let expected = hex!("
//!     077709362c2e32df0ddc3f0dc47bba63
//!     90b6c73bb50f9c3122ec844ad7c2b3e5
//! ");
//! assert_eq!(prk[..], expected[..]);
//! ```
//!
//! If you already have a strong key to work from (uniformly-distributed and
//! long enough), you can save a tiny amount of time by skipping the extract
//! step. In this case, you pass a Pseudo-Random Key (PRK) into the
//! [`Hkdf::from_prk`] constructor, then use the resulting [`Hkdf`] object
//! as usual.
//!
//! ```rust
//! # use sha2::Sha256;
//! # use hkdf::Hkdf;
//! # use hex_literal::hex;
//! # use hmac::Hmac;
//! # let salt = hex!("000102030405060708090a0b0c");
//! # let info = hex!("f0f1f2f3f4f5f6f7f8f9");
//! let prk = hex!("
//!     077709362c2e32df0ddc3f0dc47bba63
//!     90b6c73bb50f9c3122ec844ad7c2b3e5
//! ");
//!
//! let hk = Hkdf::<Hmac<Sha256>>::from_prk(&prk).expect("PRK should be large enough");
//! let mut okm = [0u8; 42];
//! hk.expand(&info, &mut okm)
//!     .expect("42 is a valid length for Sha256 to output");
//!
//! let expected = hex!("
//!     3cb25f25faacd57a90434f64d0362f2a
//!     2d2d0a90cf1a5a4c5db02d56ecc4c5bf
//!     34007208d5b887185865
//! ");
//! assert_eq!(okm[..], expected[..]);
//! ```
//!
//! [1]: https://tools.ietf.org/html/rfc5869

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg",
    html_root_url = "https://docs.rs/hkdf/0.12.0"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "std")]
extern crate std;

use core::fmt;
use hmac::digest::{generic_array::typenum::Unsigned, FixedOutput, KeyInit, Output, Update};
#[cfg(feature = "std")]
use std::fmt::Debug;

/// Error that is returned when supplied pseudorandom key (PRK) is not long enough.
#[derive(Copy, Clone, Debug)]
pub struct InvalidPrkLength;

/// Structure for InvalidLength, used for output error handling.
#[derive(Copy, Clone, Debug)]
pub struct InvalidLength;

/// Structure representing the streaming context of an HKDF-Extract operation
/// ```rust
/// # use hkdf::{Hkdf, HkdfExtract};
/// # use sha2::Sha256;
/// # use hmac::Hmac;
///
/// let mut extract_ctx = HkdfExtract::<Hmac<Sha256>>::new(Some(b"mysalt"));
/// extract_ctx.input_ikm(b"hello");
/// extract_ctx.input_ikm(b" world");
/// let (streamed_res, _) = extract_ctx.finalize();
///
/// let (oneshot_res, _) = Hkdf::<Hmac<Sha256>>::extract(Some(b"mysalt"), b"hello world");
/// assert_eq!(streamed_res, oneshot_res);
/// ```
#[derive(Clone)]
pub struct HkdfExtract<H: Update + KeyInit + FixedOutput + Clone> {
    hmac: H,
}

impl<H: Update + KeyInit + FixedOutput + Clone> HkdfExtract<H> {
    /// Initiates the HKDF-Extract context with the given optional salt
    pub fn new(salt: Option<&[u8]>) -> HkdfExtract<H> {
        let default_salt = Output::<H>::default();
        let salt = salt.unwrap_or(&default_salt);
        let hmac = H::new_from_slice(salt).expect("HMAC can take a key of any size");
        HkdfExtract { hmac }
    }

    /// Feeds in additional input key material to the HKDF-Extract context
    pub fn input_ikm(&mut self, ikm: &[u8]) {
        self.hmac.update(ikm);
    }

    /// Completes the HKDF-Extract operation, returning both the generated pseudorandom key and
    /// `Hkdf` struct for expanding.
    pub fn finalize(self) -> (Output<H>, Hkdf<H>) {
        let prk = self.hmac.finalize_fixed();
        let hkdf = Hkdf::from_prk(&prk).expect("PRK size is correct");
        (prk, hkdf)
    }
}

#[cfg(feature = "std")]
impl<H: Update + KeyInit + FixedOutput + Clone + Debug> fmt::Debug for HkdfExtract<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HkdfExtract<{:?}> {{ ... }}", &self.hmac)
    }
}

/// Structure representing the HKDF, capable of HKDF-Expand and HKDF-Extract operations.
#[derive(Clone)]
pub struct Hkdf<H: Update + KeyInit + FixedOutput + Clone> {
    hmac: H,
}

impl<H: Update + KeyInit + FixedOutput + Clone> Hkdf<H> {
    /// Convenience method for [`extract`][Hkdf::extract] when the generated
    /// pseudorandom key can be ignored and only HKDF-Expand operation is needed. This is the most
    /// common constructor.
    pub fn new(salt: Option<&[u8]>, ikm: &[u8]) -> Hkdf<H> {
        let (_, hkdf) = Hkdf::extract(salt, ikm);
        hkdf
    }

    /// Create `Hkdf` from an already cryptographically strong pseudorandom key
    /// as per section 3.3 from RFC5869.
    pub fn from_prk(prk: &[u8]) -> Result<Hkdf<H>, InvalidPrkLength> {
        // section 2.3 specifies that prk must be "at least HashLen octets"
        if prk.len() < H::OutputSize::to_usize() {
            return Err(InvalidPrkLength);
        }

        Ok(Hkdf {
            hmac: H::new_from_slice(prk).expect("HMAC can take a key of any size"),
        })
    }

    /// The RFC5869 HKDF-Extract operation returning both the generated
    /// pseudorandom key and `Hkdf` struct for expanding.
    pub fn extract(salt: Option<&[u8]>, ikm: &[u8]) -> (Output<H>, Hkdf<H>) {
        let mut extract_ctx = HkdfExtract::new(salt);
        extract_ctx.input_ikm(ikm);
        extract_ctx.finalize()
    }

    /// The RFC5869 HKDF-Expand operation. This is equivalent to calling
    /// [`expand`][Hkdf::extract] with the `info` argument set equal to the
    /// concatenation of all the elements of `info_components`.
    pub fn expand_multi_info(
        &self,
        info_components: &[&[u8]],
        okm: &mut [u8],
    ) -> Result<(), InvalidLength> {
        let mut prev: Option<Output<H>> = None;

        let chunk_len = H::OutputSize::USIZE;
        if okm.len() > chunk_len * 255 {
            return Err(InvalidLength);
        }

        for (block_n, block) in okm.chunks_mut(chunk_len).enumerate() {
            let mut hmac = self.hmac.clone();

            if let Some(ref prev) = prev {
                hmac.update(prev)
            };

            // Feed in the info components in sequence. This is equivalent to feeding in the
            // concatenation of all the info components
            for info in info_components {
                hmac.update(info);
            }

            hmac.update(&[block_n as u8 + 1]);

            let output = hmac.finalize_fixed();

            let block_len = block.len();
            block.copy_from_slice(&output[..block_len]);

            prev = Some(output);
        }

        Ok(())
    }

    /// The RFC5869 HKDF-Expand operation
    ///
    /// If you don't have any `info` to pass, use an empty slice.
    pub fn expand(&self, info: &[u8], okm: &mut [u8]) -> Result<(), InvalidLength> {
        self.expand_multi_info(&[info], okm)
    }
}

#[cfg(feature = "std")]
impl<H: Update + KeyInit + FixedOutput + Clone + Debug> fmt::Debug for Hkdf<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HkdfExtract<{:?}> {{ ... }}", &self.hmac)
    }
}

impl fmt::Display for InvalidPrkLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("invalid pseudorandom key length, too short")
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl ::std::error::Error for InvalidPrkLength {}

impl fmt::Display for InvalidLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("invalid number of blocks, too large output")
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl ::std::error::Error for InvalidLength {}
