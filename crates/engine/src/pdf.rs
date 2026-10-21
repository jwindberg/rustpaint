//! Pages as a PDF, for printing: each canvas a 1-bit image at 72 dpi (one
//! pixel to a point), centered on a US Letter page, so MacPaint's 576 x
//! 720 page prints at its real 8 x 10 inches. The image data is packed
//! with PackBits, which is what PDF's RunLengthDecode filter reads.

use crate::canvas::Canvas;
use crate::macpaint_file::pack_bits;

/// US Letter, in points.
const PAGE_WIDTH: i32 = 612;
const PAGE_HEIGHT: i32 = 792;

/// A PDF with one page per canvas.
pub fn write_pdf(pages: &[Canvas]) -> Vec<u8> {
    let mut pdf = Pdf::default();
    pdf.out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");

    // Objects 1 and 2 are the catalog and the page tree; each page then
    // takes three: the page, its contents and its image.
    let page_ids: Vec<usize> = (0..pages.len()).map(|i| 3 + 3 * i).collect();
    pdf.object(1, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();
    let tree = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.join(" "),
        pages.len()
    );
    pdf.object(2, tree.as_bytes());

    for (page, &id) in pages.iter().zip(&page_ids) {
        let (width, height) = (page.width(), page.height());
        // Centered, and scaled down only if it's bigger than the paper.
        let scale = (PAGE_WIDTH as f64 / width as f64)
            .min(PAGE_HEIGHT as f64 / height as f64)
            .min(1.0);
        let (shown_width, shown_height) = (width as f64 * scale, height as f64 * scale);
        let left = (PAGE_WIDTH as f64 - shown_width) / 2.0;
        let bottom = (PAGE_HEIGHT as f64 - shown_height) / 2.0;

        let page_object = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] \
             /Resources << /XObject << /Im0 {} 0 R >> >> /Contents {} 0 R >>",
            id + 2,
            id + 1
        );
        pdf.object(id, page_object.as_bytes());

        let contents =
            format!("q {shown_width:.2} 0 0 {shown_height:.2} {left:.2} {bottom:.2} cm /Im0 Do Q");
        pdf.stream(id + 1, "", contents.as_bytes());

        // Rows of bits, 1 for black; Decode [1 0] says 1 is black, where
        // DeviceGray would make it white.
        let row_bytes = (width as usize).div_ceil(8);
        let mut packed = Vec::new();
        let mut row = vec![0u8; row_bytes];
        for y in 0..height {
            row.fill(0);
            for x in 0..width {
                if page.get(x, y) {
                    row[x as usize / 8] |= 0x80 >> (x % 8);
                }
            }
            packed.extend(pack_bits(&row));
        }
        packed.push(128); // end of data
        let dictionary = format!(
            "/Type /XObject /Subtype /Image /Width {width} /Height {height} \
             /ColorSpace /DeviceGray /BitsPerComponent 1 /Decode [1 0] \
             /Filter /RunLengthDecode"
        );
        pdf.stream(id + 2, &dictionary, &packed);
    }

    let count = 3 + 3 * pages.len();
    let xref = pdf.out.len();
    let mut table = format!("xref\n0 {count}\n0000000000 65535 f \n");
    for offset in &pdf.offsets[1..count] {
        table.push_str(&format!("{offset:010} 00000 n \n"));
    }
    table.push_str(&format!(
        "trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    ));
    pdf.out.extend_from_slice(table.as_bytes());
    pdf.out
}

#[derive(Default)]
struct Pdf {
    out: Vec<u8>,
    /// Where each object starts, by number.
    offsets: Vec<usize>,
}

impl Pdf {
    fn begin(&mut self, id: usize) {
        if self.offsets.len() <= id {
            self.offsets.resize(id + 1, 0);
        }
        self.offsets[id] = self.out.len();
        self.out
            .extend_from_slice(format!("{id} 0 obj\n").as_bytes());
    }

    fn object(&mut self, id: usize, body: &[u8]) {
        self.begin(id);
        self.out.extend_from_slice(body);
        self.out.extend_from_slice(b"\nendobj\n");
    }

    fn stream(&mut self, id: usize, dictionary: &str, data: &[u8]) {
        self.begin(id);
        let header = format!("<< {dictionary} /Length {} >>\nstream\n", data.len());
        self.out.extend_from_slice(header.as_bytes());
        self.out.extend_from_slice(data);
        self.out.extend_from_slice(b"\nendstream\nendobj\n");
    }
}
