//! Inline images: validated web sources with the MXP `<IMAGE>` and HTML
//! encodings, plus the markup serialization used by style-preserving truncation.
use super::parser::Directive;
use anyhow::{Result, ensure};

/// Placement hints shared by MXP and HTML output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Top,
    Middle,
    Bottom,
}

impl Align {
    /// Parse the case-insensitive MXP spelling.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "left" => Self::Left,
            "right" => Self::Right,
            "top" => Self::Top,
            "middle" => Self::Middle,
            "bottom" => Self::Bottom,
            _ => return None,
        })
    }

    /// Canonical lower-case spelling, shared by markup and the MXP attribute.
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Middle => "middle",
            Self::Bottom => "bottom",
        }
    }
}

/// One inline image. The span text it decorates is the fallback shown by
/// clients that cannot display the image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Complete `http:`, `https:` or `ftp:` URL of the image file.
    pub source: String,
    /// Display width in pixels, when the author fixed it.
    pub width: Option<u32>,
    /// Display height in pixels, when the author fixed it.
    pub height: Option<u32>,
    /// Placement relative to the surrounding text.
    pub align: Option<Align>,
}

impl Image {
    /// Build an image from a bare URL, as Markdown supplies it.
    pub fn from_url(source: &str) -> Result<Self> {
        let image = Self {
            source: source.to_owned(),
            width: None,
            height: None,
            align: None,
        };
        image.validate()?;
        Ok(image)
    }

    /// Decode `[image="URL" width=N height=N align=NAME]` directives; the
    /// first directive is the source.
    pub(super) fn parse(ds: &[Directive]) -> Result<Self> {
        let source = ds
            .first()
            .filter(|d| d.name == "image")
            .ok_or_else(|| anyhow::anyhow!("missing image source"))?;
        ensure!(source.quoted, "image source must be quoted");
        let mut image = Self::from_url(source.value.as_deref().unwrap_or(""))?;
        for d in &ds[1..] {
            let value = d.value.as_deref().unwrap_or("");
            match d.name.as_str() {
                "width" | "height" => {
                    ensure!(
                        !d.quoted && !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
                        "{} must be an unsigned decimal integer",
                        d.name
                    );
                    let pixels: u32 = value.parse()?;
                    ensure!((1..=4096).contains(&pixels), "{} is out of range", d.name);
                    if d.name == "width" {
                        image.width = Some(pixels);
                    } else {
                        image.height = Some(pixels);
                    }
                }
                "align" => {
                    ensure!(!d.quoted, "align cannot be quoted");
                    image.align =
                        Some(Align::parse(value).ok_or_else(|| anyhow::anyhow!("invalid align"))?);
                }
                other => anyhow::bail!("unknown image property {other}"),
            }
        }
        Ok(image)
    }

    /// Images follow the external link policy and must name a file.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            super::links::external(&self.source),
            "image source must be a percent-encoded http, https or ftp URL"
        );
        ensure!(
            !self.source.contains(['?', '#']),
            "image source cannot carry a query or fragment"
        );
        let (_, body) = self.source.split_once("://").unwrap_or(("", ""));
        ensure!(
            body.split_once('/')
                .is_some_and(|(host, path)| !host.is_empty() && !path.is_empty()),
            "image source must include a host and a file path"
        );
        ensure!(
            !self.file_name().is_empty(),
            "image source must end with a file name"
        );
        Ok(())
    }

    /// The final path segment: MXP clients cache images under this name.
    pub fn file_name(&self) -> &str {
        self.source.rsplit('/').next().unwrap_or("")
    }

    /// Everything through the final `/`, where MXP clients download the file.
    pub fn base_url(&self) -> &str {
        &self.source[..self.source.len() - self.file_name().len()]
    }

    /// The complete MXP tag, including the one-shot secure mode switch.
    pub fn mxp_tag(&self) -> String {
        let mut tag = format!(
            "<IMAGE {} URL={}",
            self.file_name(),
            crate::telnet::mxp::quote(self.base_url())
        );
        if let Some(width) = self.width {
            tag.push_str(&format!(" W={width}"));
        }
        if let Some(height) = self.height {
            tag.push_str(&format!(" H={height}"));
        }
        if let Some(align) = self.align {
            tag.push_str(&format!(" ALIGN={}", align.name()));
        }
        tag.push('>');
        crate::telnet::mxp::secure_tag(&tag)
    }

    /// An escaped HTML `<img>` element with the span text as its alternative text.
    pub fn html(&self, alt: &str) -> String {
        let escape = super::render::html_escape;
        let mut out = format!(
            "<img src=\"{}\" alt=\"{}\"",
            escape(&self.source),
            escape(alt)
        );
        if let Some(width) = self.width {
            out.push_str(&format!(" width=\"{width}\""));
        }
        if let Some(height) = self.height {
            out.push_str(&format!(" height=\"{height}\""));
        }
        if let Some(align) = self.align {
            out.push_str(&format!(" class=\"mux-align-{}\"", align.name()));
        }
        out.push('>');
        out
    }

    /// Opening-tag directives that reproduce this image in bracket markup.
    pub fn markup(&self) -> String {
        let mut out = format!("image={}", serde_json::to_string(&self.source).unwrap());
        if let Some(width) = self.width {
            out.push_str(&format!(" width={width}"));
        }
        if let Some(height) = self.height {
            out.push_str(&format!(" height={height}"));
        }
        if let Some(align) = self.align {
            out.push_str(&format!(" align={}", align.name()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_split_into_cache_name_and_download_base() {
        let image = Image::from_url("https://cdn.example.com/mechs/atlas.png").unwrap();
        assert_eq!(image.file_name(), "atlas.png");
        assert_eq!(image.base_url(), "https://cdn.example.com/mechs/");
        assert_eq!(
            image.mxp_tag(),
            "\x1b[4z<IMAGE atlas.png URL=\"https://cdn.example.com/mechs/\">"
        );
        for bad in [
            "https://cdn.example.com",
            "https://cdn.example.com/",
            "https://cdn.example.com/dir/",
            "https://cdn.example.com/a.png?size=1",
            "https://cdn.example.com/a.png#x",
            "javascript:alert(1)",
            "https://cdn.example.com/a b.png",
            "",
        ] {
            assert!(Image::from_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn directives_set_dimensions_and_alignment() {
        let ds = super::super::parser::directives(
            "image=\"https://x.example/i/a.png\" width=200 height=100 align=Right",
        )
        .unwrap();
        let image = Image::parse(&ds).unwrap();
        assert_eq!((image.width, image.height), (Some(200), Some(100)));
        assert_eq!(image.align, Some(Align::Right));
        assert_eq!(
            image.mxp_tag(),
            "\x1b[4z<IMAGE a.png URL=\"https://x.example/i/\" W=200 H=100 ALIGN=right>"
        );
        assert_eq!(
            image.html("Atlas"),
            "<img src=\"https://x.example/i/a.png\" alt=\"Atlas\" width=\"200\" height=\"100\" class=\"mux-align-right\">"
        );
        assert_eq!(
            image.markup(),
            "image=\"https://x.example/i/a.png\" width=200 height=100 align=right"
        );
        for bad in [
            "image=https://x.example/i/a.png",
            "image=\"https://x.example/i/a.png\" width=0",
            "image=\"https://x.example/i/a.png\" width=\"20\"",
            "image=\"https://x.example/i/a.png\" height=5000",
            "image=\"https://x.example/i/a.png\" align=center",
            "image=\"https://x.example/i/a.png\" bold",
        ] {
            let ds = super::super::parser::directives(bad).unwrap();
            assert!(Image::parse(&ds).is_err(), "{bad}");
        }
    }
}
