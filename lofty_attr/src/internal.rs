// Items that only pertain to internal usage of lofty_attr

use crate::FieldContents;

use std::collections::HashMap;

use quote::quote;

pub(crate) fn opt_internal_file_type(
	struct_name: String,
) -> Option<(proc_macro2::TokenStream, bool)> {
	const LOFTY_FILE_TYPES: [&str; 10] = [
		"AIFF", "APE", "FLAC", "MPEG", "MP4", "Opus", "Vorbis", "Speex", "WAV", "WavPack",
	];

	const ID3V2_STRIPPABLE: [&str; 1] = ["APE"];

	let stripped = struct_name.strip_suffix("File");
	if let Some(prefix) = stripped {
		if let Some(pos) = LOFTY_FILE_TYPES
			.iter()
			.position(|p| p.eq_ignore_ascii_case(prefix))
		{
			let file_ty = LOFTY_FILE_TYPES[pos];
			let tt = file_ty.parse::<proc_macro2::TokenStream>().unwrap();

			return Some((tt, ID3V2_STRIPPABLE.contains(&file_ty)));
		}
	}

	None
}

pub(crate) fn init_write_lookup(
	id3v2_strippable: bool,
) -> HashMap<&'static str, proc_macro2::TokenStream> {
	let mut map = HashMap::new();

	macro_rules! insert {
		($map:ident, $key:path, $val:block) => {
			$map.insert(stringify!($key), quote! { $val })
		};
	}

	insert!(map, APE, {
		crate::ape::tag::ApeTagRef {
			read_only: false,
			items: crate::ape::tag::tagitems_into_ape(tag.items()),
		}
		.write_to(data)
	});

	insert!(map, ID3v1, {
		Into::<crate::id3::v1::tag::Id3v1TagRef<'_>>::into(tag).write_to(data)
	});

	if id3v2_strippable {
		insert!(map, ID3v2, {
			crate::id3::v2::tag::Id3v2TagRef::empty().write_to(data)
		});
	} else {
		insert!(map, ID3v2, {
			crate::id3::v2::tag::Id3v2TagRef {
				flags: crate::id3::v2::ID3v2TagFlags::default(),
				frames: crate::id3::v2::tag::tag_frames(tag),
			}
			.write_to(data)
		});
	}

	insert!(map, RIFFInfo, {
		crate::iff::wav::tag::RIFFInfoListRef::new(crate::iff::wav::tag::tagitems_into_riff(
			tag.items(),
		))
		.write_to(data)
	});

	insert!(map, AIFFText, {
		crate::iff::aiff::tag::AiffTextChunksRef {
			name: tag.get_string(&crate::tag::item::ItemKey::TrackTitle),
			author: tag.get_string(&crate::tag::item::ItemKey::TrackArtist),
			copyright: tag.get_string(&crate::tag::item::ItemKey::CopyrightMessage),
			annotations: Some(tag.get_strings(&crate::tag::item::ItemKey::Comment)),
			comments: None,
		}
		.write_to(data)
	});

	map
}

pub(crate) fn write_module(
	fields: &[FieldContents],
	lookup: HashMap<&'static str, proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
	let applicable_formats = fields.iter().map(|f| {
		let tag_ty =
			syn::parse_str::<syn::Path>(&format!("crate::tag::TagType::{}", &f.tag_type)).unwrap();

		let features = f.cfg_features.iter();

		let block = lookup.get(&*tag_ty.segments[3].ident.to_string()).unwrap();

		quote! {
			#( #features )*
			#tag_ty => #block,
		}
	});

	quote! {
		pub(crate) mod write {
			#[allow(unused_variables)]
			pub(crate) fn write_to(data: &mut std::fs::File, tag: &crate::tag::Tag) -> crate::error::Result<()> {
				match tag.tag_type() {
					#( #applicable_formats )*
					_ => crate::macros::err!(UnsupportedTag),
				}
			}
		}
	}
}