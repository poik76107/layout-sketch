use crate::scene::Point;

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Move(Point),
    Line(Point),
    Quadratic(Point, Point),
    Cubic(Point, Point, Point),
    Close,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPath {
    pub segments: Vec<Segment>,
    pub all_closed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    pub offset: usize,
    pub detail: String,
}
struct Parser<'a> {
    input: &'a str,
    offset: usize,
}
impl Parser<'_> {
    fn error(&self, offset: usize, detail: &str) -> PathError {
        PathError {
            offset,
            detail: detail.into(),
        }
    }
    fn skip_space(&mut self) {
        while self
            .input
            .as_bytes()
            .get(self.offset)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.offset += 1;
        }
    }
    fn byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.offset).copied()
    }
    fn number(&mut self, first: bool) -> Result<f64, PathError> {
        self.skip_space();
        if self.byte() == Some(b',') {
            let comma = self.offset;
            if first {
                return Err(self.error(comma, "unexpected comma"));
            }
            self.offset += 1;
            self.skip_space();
            if !self
                .byte()
                .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.'))
            {
                return Err(self.error(comma, "dangling comma"));
            }
        }
        let start = self.offset;
        if self.byte().is_some_and(|b| matches!(b, b'+' | b'-')) {
            self.offset += 1;
        }
        let integer_start = self.offset;
        while self.byte().is_some_and(|b| b.is_ascii_digit()) {
            self.offset += 1;
        }
        let mut digits = self.offset - integer_start;
        if self.byte() == Some(b'.') {
            self.offset += 1;
            let fractional_start = self.offset;
            while self.byte().is_some_and(|b| b.is_ascii_digit()) {
                self.offset += 1;
            }
            digits += self.offset - fractional_start;
        }
        if digits == 0 {
            return Err(self.error(start, "expected a number"));
        }
        if self.byte().is_some_and(|b| matches!(b, b'e' | b'E')) {
            self.offset += 1;
            if self.byte().is_some_and(|b| matches!(b, b'+' | b'-')) {
                self.offset += 1;
            }
            let exponent_start = self.offset;
            while self.byte().is_some_and(|b| b.is_ascii_digit()) {
                self.offset += 1;
            }
            if self.offset == exponent_start {
                return Err(self.error(start, "malformed exponent"));
            }
        }
        let value = self.input[start..self.offset]
            .parse::<f64>()
            .map_err(|_| self.error(start, "invalid number"))?;
        if !value.is_finite() {
            return Err(self.error(start, "expected a finite number"));
        }
        Ok(value)
    }
    fn points<const N: usize>(&mut self) -> Result<[f64; N], PathError> {
        let mut values = [0.0; N];
        for (index, value) in values.iter_mut().enumerate() {
            *value = self.number(index == 0)?;
        }
        Ok(values)
    }
}

pub fn parse_path(input: &str) -> Result<ParsedPath, PathError> {
    let mut parser = Parser { input, offset: 0 };
    let mut segments = Vec::new();
    let mut has_move = false;
    let mut has_segment = false;
    let mut is_closed = false;
    let mut all_closed = true;
    loop {
        parser.skip_space();
        let Some(command) = parser.byte() else {
            break;
        };
        let position = parser.offset;
        if command == b',' {
            return Err(parser.error(position, "dangling comma"));
        }
        if !matches!(command, b'M' | b'L' | b'Q' | b'C' | b'Z') {
            return Err(parser.error(position, "unsupported command or trailing input"));
        }
        if !has_move && command != b'M' {
            return Err(parser.error(position, "expected initial M"));
        }
        if is_closed && command != b'M' {
            return Err(parser.error(position, "expected M after Z"));
        }
        if command == b'M' && has_move && !has_segment {
            return Err(parser.error(position, "subcontour needs a segment"));
        }
        if command == b'Z' && !has_segment {
            return Err(parser.error(position, "subcontour needs a segment"));
        }
        parser.offset += 1;
        let segment = match command {
            b'M' => {
                if has_move && !is_closed {
                    all_closed = false;
                }
                let [x, y] = parser.points()?;
                has_move = true;
                has_segment = false;
                is_closed = false;
                Segment::Move([x, y])
            }
            b'L' => {
                let [x, y] = parser.points()?;
                has_segment = true;
                Segment::Line([x, y])
            }
            b'Q' => {
                let [x1, y1, x, y] = parser.points()?;
                has_segment = true;
                Segment::Quadratic([x1, y1], [x, y])
            }
            b'C' => {
                let [x1, y1, x2, y2, x, y] = parser.points()?;
                has_segment = true;
                Segment::Cubic([x1, y1], [x2, y2], [x, y])
            }
            _ => {
                is_closed = true;
                Segment::Close
            }
        };
        segments
            .try_reserve(1)
            .map_err(|_| parser.error(position, "path allocation failed"))?;
        segments.push(segment);
    }
    if !has_move {
        return Err(parser.error(parser.offset, "expected initial M"));
    }
    if !has_segment {
        return Err(parser.error(parser.offset, "subcontour needs a segment"));
    }
    if !is_closed {
        all_closed = false;
    }
    Ok(ParsedPath {
        segments,
        all_closed,
    })
}

#[cfg(test)]
mod tests;
