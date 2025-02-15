use windows::{
    core::HSTRING,
    Win32::{
        Foundation::{BOOL, RECT},
        Graphics::{Direct2D::*, DirectWrite::*},
    },
};

#[derive(Debug)]
#[allow(dead_code)]
enum Error {
    Io(std::io::Error),
    Windows(windows::core::Error),
    Encoding(png::EncodingError),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
impl From<png::EncodingError> for Error {
    fn from(e: png::EncodingError) -> Self {
        Error::Encoding(e)
    }
}
impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Error::Windows(e)
    }
}

type Result<T> = std::result::Result<T, Error>;

struct Glyph {
    texturebounds: RECT,
    texture_type: DWRITE_TEXTURE_TYPE,
    alpha_values: Vec<u8>,
}

fn draw_glyph(antialiasmode: DWRITE_TEXT_ANTIALIAS_MODE, ch: u32) -> Result<Option<Glyph>> {
    let dwrite_factory: IDWriteFactory5 =
        unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
    let font_collection = {
        let mut fontcollection: Option<IDWriteFontCollection1> = None;
        unsafe { dwrite_factory.GetSystemFontCollection(false, &mut fontcollection, false) }?;
        fontcollection
    };
    let _d2d1_factory: ID2D1Factory =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_MULTI_THREADED, None) }?;
    let mut font_face: Option<IDWriteFontFace3> = None;
    if let Some(fontcollection) = &font_collection {
        let font_set = unsafe { fontcollection.GetFontSet() }?;
        let font_set = unsafe {
            font_set.GetMatchingFonts(
                &HSTRING::from("Bizin Gothic"),
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )
        }?;
        font_face = Some(unsafe { font_set.GetFontFaceReference(0)?.CreateFontFace() }?);
    }
    let mut glyph_indices = vec![0_u16, 1];
    let a = font_face.map(|font_face| -> Result<Glyph> {
        let advance = [0.0f32];
        let offset = [DWRITE_GLYPH_OFFSET::default()];
        let codepoints = [ch];
        let codepointcount = codepoints.len() as u32;
        unsafe {
            font_face.GetGlyphIndices(
                codepoints.as_ptr(),
                codepointcount,
                glyph_indices.as_mut_ptr(),
            )
        }?;
        let glyph_run = DWRITE_GLYPH_RUN {
            fontFace: unsafe { std::mem::transmute_copy(&font_face) },
            fontEmSize: 24.0_f32,
            glyphCount: 1,
            glyphIndices: glyph_indices.as_ptr(),
            glyphAdvances: advance.as_ptr(),
            glyphOffsets: offset.as_ptr(),
            isSideways: BOOL(0),
            bidiLevel: 0,
        };
        let glyph_run_analysis = unsafe {
            dwrite_factory.CreateGlyphRunAnalysis(
                &glyph_run,
                None,
                DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC,
                DWRITE_MEASURING_MODE_NATURAL,
                DWRITE_GRID_FIT_MODE_DEFAULT,
                antialiasmode,
                0.0_f32,
                0.0_f32,
            )
        }?;
        let texturetype = if antialiasmode == DWRITE_TEXT_ANTIALIAS_MODE_CLEARTYPE {
            DWRITE_TEXTURE_CLEARTYPE_3x1
        } else {
            DWRITE_TEXTURE_ALIASED_1x1
        };
        let texturebounds = unsafe { glyph_run_analysis.GetAlphaTextureBounds(texturetype) }?;
        let mut alphavalues = vec![
            0_u8;
            if texturetype == DWRITE_TEXTURE_CLEARTYPE_3x1 {
                3
            } else {
                1
            } * ((texturebounds.right - texturebounds.left)
                * (texturebounds.bottom - texturebounds.top))
                as usize
        ];
        unsafe {
            glyph_run_analysis.CreateAlphaTexture(texturetype, &texturebounds, &mut alphavalues)
        }?;
        Ok(Glyph {
            texturebounds,
            texture_type: texturetype,
            alpha_values: alphavalues,
        })
    });
    a.transpose()
}

fn main() -> Result<()> {
    for antialiasmode in [
        DWRITE_TEXT_ANTIALIAS_MODE_CLEARTYPE,
        DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE,
    ] {
        let s = "namespace";
        let glyphs = s
            .chars()
            .filter_map(|ch| draw_glyph(antialiasmode, ch as u32).transpose())
            .collect::<Result<Vec<_>>>()?;
        for (ch, glyph) in s.chars().zip(glyphs.iter()) {
            let w = std::fs::File::create(format!("{}_{ch}.png", antialiasmode.0))?;
            let width = (glyph.texturebounds.right - glyph.texturebounds.left) as u32;
            let height = (glyph.texturebounds.bottom - glyph.texturebounds.top) as u32;
            let mut enc = png::Encoder::new(&w, width, height);
            enc.set_color(if glyph.texture_type == DWRITE_TEXTURE_CLEARTYPE_3x1 {
                png::ColorType::Rgb
            } else {
                png::ColorType::Grayscale
            });
            enc.write_header()?.write_image_data(&glyph.alpha_values)?;
        }
    }
    Ok(())
}
