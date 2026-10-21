//! MacPaint's tools.

/// MacPaint's tool numbers, which are also the tools' positions in the
/// palette, two per row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Lasso,
    Select,
    Grabber,
    Text,
    PaintBucket,
    Spray,
    Brush,
    Pencil,
    Line,
    Eraser,
    Rect,
    FilledRect,
    RoundRect,
    FilledRoundRect,
    Oval,
    FilledOval,
    Freeform,
    FilledFreeform,
    Polygon,
    FilledPolygon,
}

pub const TOOL_COUNT: usize = 20;

impl Tool {
    const ALL: [Tool; TOOL_COUNT] = [
        Tool::Lasso,
        Tool::Select,
        Tool::Grabber,
        Tool::Text,
        Tool::PaintBucket,
        Tool::Spray,
        Tool::Brush,
        Tool::Pencil,
        Tool::Line,
        Tool::Eraser,
        Tool::Rect,
        Tool::FilledRect,
        Tool::RoundRect,
        Tool::FilledRoundRect,
        Tool::Oval,
        Tool::FilledOval,
        Tool::Freeform,
        Tool::FilledFreeform,
        Tool::Polygon,
        Tool::FilledPolygon,
    ];

    pub fn from_index(index: usize) -> Option<Tool> {
        Self::ALL.get(index).copied()
    }

    pub fn is_shape(self) -> bool {
        (Tool::Rect as usize..=Tool::FilledOval as usize).contains(&(self as usize))
    }

    pub fn is_selection(self) -> bool {
        matches!(self, Tool::Lasso | Tool::Select)
    }

    pub fn is_polygon(self) -> bool {
        matches!(self, Tool::Polygon | Tool::FilledPolygon)
    }

    /// The tools MacPaint's Grid applies to (its cheapSet: tools 1, 3, 8,
    /// 10-15, 18 and 19).
    pub fn uses_grid(self) -> bool {
        matches!(
            self,
            Tool::Select | Tool::Text | Tool::Line | Tool::Polygon | Tool::FilledPolygon
        ) || self.is_shape()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_match_macpaint() {
        for (index, tool) in Tool::ALL.iter().enumerate() {
            assert_eq!(*tool as usize, index);
            assert_eq!(Tool::from_index(index), Some(*tool));
        }
        assert_eq!(Tool::from_index(TOOL_COUNT), None);
        let grid: Vec<usize> = Tool::ALL
            .iter()
            .filter(|t| t.uses_grid())
            .map(|&t| t as usize)
            .collect();
        assert_eq!(grid, [1, 3, 8, 10, 11, 12, 13, 14, 15, 18, 19]);
    }
}
