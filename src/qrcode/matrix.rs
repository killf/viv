use crate::qrcode::tables;

/// A QR code module matrix.
///
/// Manages the grid of black/white modules that make up a QR code, including
/// functional patterns, data bit placement, masking, and format/version info.
pub struct QrMatrix {
    version: u8,
    size: usize,
    modules: Vec<Vec<bool>>,
    is_function: Vec<Vec<bool>>,
}

impl QrMatrix {
    /// Create matrix with all functional patterns placed and format/version areas reserved.
    pub fn new(version: u8) -> Self {
        let size = version as usize * 4 + 17;
        let modules = vec![vec![false; size]; size];
        let is_function = vec![vec![false; size]; size];
        let mut m = QrMatrix {
            version,
            size,
            modules,
            is_function,
        };

        m.place_finder_patterns();
        m.place_separators();
        m.place_timing_patterns();
        m.place_alignment_patterns();
        m.place_dark_module();
        m.reserve_format_areas();
        m.reserve_version_areas();

        m
    }

    /// Full build: new + place data + best mask + format/version info.
    pub fn build(version: u8, data: &[u8]) -> Self {
        let mut m = Self::new(version);
        m.place_data_bits(data);

        let best_mask = m.select_best_mask();
        m.apply_mask(best_mask);
        m.write_format_info(best_mask);
        m.write_version_info();

        m
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn get(&self, row: usize, col: usize) -> bool {
        self.modules[row][col]
    }

    pub fn modules(&self) -> &Vec<Vec<bool>> {
        &self.modules
    }

    // -----------------------------------------------------------------------
    // Functional patterns
    // -----------------------------------------------------------------------

    fn place_finder_pattern(&mut self, row: usize, col: usize) {
        for r in 0..7 {
            for c in 0..7 {
                let is_border = r == 0 || r == 6 || c == 0 || c == 6;
                let is_center = (2..=4).contains(&r) && (2..=4).contains(&c);
                let black = is_border || is_center;
                self.modules[row + r][col + c] = black;
                self.is_function[row + r][col + c] = true;
            }
        }
    }

    fn place_finder_patterns(&mut self) {
        self.place_finder_pattern(0, 0);
        self.place_finder_pattern(0, self.size - 7);
        self.place_finder_pattern(self.size - 7, 0);
    }

    fn place_separators(&mut self) {
        let s = self.size;

        // Top-left: row 7 (cols 0-7) and col 7 (rows 0-7)
        for i in 0..8 {
            self.set_function(7, i, false);
            self.set_function(i, 7, false);
        }

        // Top-right: row 7 (cols size-8 to size-1) and col size-8 (rows 0-7)
        for i in 0..8 {
            self.set_function(7, s - 8 + i, false);
            self.set_function(i, s - 8, false);
        }

        // Bottom-left: row size-8 (cols 0-7) and col 7 (rows size-8 to size-1)
        for i in 0..8 {
            self.set_function(s - 8, i, false);
            self.set_function(s - 8 + i, 7, false);
        }
    }

    fn place_timing_patterns(&mut self) {
        let s = self.size;
        for i in 8..s - 8 {
            let black = i % 2 == 0;
            // Row 6
            if !self.is_function[6][i] {
                self.modules[6][i] = black;
                self.is_function[6][i] = true;
            }
            // Col 6
            if !self.is_function[i][6] {
                self.modules[i][6] = black;
                self.is_function[i][6] = true;
            }
        }
    }

    fn place_alignment_patterns(&mut self) {
        if self.version < 2 {
            return;
        }
        let positions = tables::ALIGNMENT_POSITIONS[self.version as usize - 1];
        for &r in positions {
            for &c in positions {
                // Skip if it would overlap a finder pattern
                if self.is_function[r as usize][c as usize] {
                    continue;
                }
                self.place_alignment_pattern(r as usize, c as usize);
            }
        }
    }

    fn place_alignment_pattern(&mut self, center_row: usize, center_col: usize) {
        for dr in 0..5 {
            for dc in 0..5 {
                let r = center_row - 2 + dr;
                let c = center_col - 2 + dc;
                let is_border = dr == 0 || dr == 4 || dc == 0 || dc == 4;
                let is_center = dr == 2 && dc == 2;
                let black = is_border || is_center;
                self.modules[r][c] = black;
                self.is_function[r][c] = true;
            }
        }
    }

    fn place_dark_module(&mut self) {
        let row = 4 * self.version as usize + 9;
        self.modules[row][8] = true;
        self.is_function[row][8] = true;
    }

    fn reserve_format_areas(&mut self) {
        let s = self.size;
        // Row 8, cols 0-8
        for c in 0..9 {
            self.is_function[8][c] = true;
        }
        // Row 8, cols size-8 to size-1
        for c in (s - 8)..s {
            self.is_function[8][c] = true;
        }
        // Col 8, rows 0-8
        for r in 0..9 {
            self.is_function[r][8] = true;
        }
        // Col 8, rows size-8 to size-1
        for r in (s - 8)..s {
            self.is_function[r][8] = true;
        }
    }

    fn reserve_version_areas(&mut self) {
        if self.version < 7 {
            return;
        }
        let s = self.size;
        // Bottom-left: 6x3 block — rows (s-11) to (s-9), cols 0 to 5
        for r in (s - 11)..=(s - 9) {
            for c in 0..6 {
                self.is_function[r][c] = true;
            }
        }
        // Top-right: 3x6 block — rows 0 to 5, cols (s-11) to (s-9)
        for r in 0..6 {
            for c in (s - 11)..=(s - 9) {
                self.is_function[r][c] = true;
            }
        }
    }

    fn set_function(&mut self, row: usize, col: usize, black: bool) {
        self.modules[row][col] = black;
        self.is_function[row][col] = true;
    }

    // -----------------------------------------------------------------------
    // Data placement
    // -----------------------------------------------------------------------

    fn place_data_bits(&mut self, data: &[u8]) {
        let total_bits = data.len() * 8;
        let mut bit_index = 0;

        // Start from rightmost column, moving left in pairs
        let mut right = self.size as isize - 1;
        let mut going_up = true;

        while right >= 0 {
            // Skip column 6 (timing pattern column)
            if right == 6 {
                right -= 1;
                continue;
            }

            let left = right - 1;

            if going_up {
                for row in (0..self.size).rev() {
                    // Right column
                    if !self.is_function[row][right as usize] {
                        if bit_index < total_bits {
                            let byte = data[bit_index / 8];
                            let bit = (byte >> (7 - (bit_index % 8))) & 1;
                            self.modules[row][right as usize] = bit == 1;
                        }
                        bit_index += 1;
                    }
                    // Left column
                    if left >= 0 && !self.is_function[row][left as usize] {
                        if bit_index < total_bits {
                            let byte = data[bit_index / 8];
                            let bit = (byte >> (7 - (bit_index % 8))) & 1;
                            self.modules[row][left as usize] = bit == 1;
                        }
                        bit_index += 1;
                    }
                }
            } else {
                for row in 0..self.size {
                    // Right column
                    if !self.is_function[row][right as usize] {
                        if bit_index < total_bits {
                            let byte = data[bit_index / 8];
                            let bit = (byte >> (7 - (bit_index % 8))) & 1;
                            self.modules[row][right as usize] = bit == 1;
                        }
                        bit_index += 1;
                    }
                    // Left column
                    if left >= 0 && !self.is_function[row][left as usize] {
                        if bit_index < total_bits {
                            let byte = data[bit_index / 8];
                            let bit = (byte >> (7 - (bit_index % 8))) & 1;
                            self.modules[row][left as usize] = bit == 1;
                        }
                        bit_index += 1;
                    }
                }
            }

            going_up = !going_up;
            right -= 2;
        }
    }

    // -----------------------------------------------------------------------
    // Masking
    // -----------------------------------------------------------------------

    fn mask_condition(mask: u8, row: usize, col: usize) -> bool {
        match mask {
            0 => (row + col).is_multiple_of(2),
            1 => row.is_multiple_of(2),
            2 => col.is_multiple_of(3),
            3 => (row + col).is_multiple_of(3),
            4 => (row / 2 + col / 3).is_multiple_of(2),
            5 => (row * col) % 2 + (row * col) % 3 == 0,
            6 => ((row * col) % 2 + (row * col) % 3).is_multiple_of(2),
            7 => ((row + col) % 2 + (row * col) % 3).is_multiple_of(2),
            _ => false,
        }
    }

    fn apply_mask(&mut self, mask: u8) {
        for row in 0..self.size {
            for col in 0..self.size {
                if !self.is_function[row][col] && Self::mask_condition(mask, row, col) {
                    self.modules[row][col] = !self.modules[row][col];
                }
            }
        }
    }

    fn select_best_mask(&self) -> u8 {
        let mut best_mask = 0u8;
        let mut best_score = u32::MAX;

        for mask in 0..8u8 {
            // Clone the matrix, apply mask and format info, then score
            let mut candidate = QrMatrix {
                version: self.version,
                size: self.size,
                modules: self.modules.clone(),
                is_function: self.is_function.clone(),
            };
            candidate.apply_mask(mask);
            candidate.write_format_info(mask);
            candidate.write_version_info();

            let score = candidate.penalty_score();
            if score < best_score {
                best_score = score;
                best_mask = mask;
            }
        }

        best_mask
    }

    // -----------------------------------------------------------------------
    // Penalty scoring
    // -----------------------------------------------------------------------

    fn penalty_score(&self) -> u32 {
        self.penalty_rule1() + self.penalty_rule2() + self.penalty_rule3() + self.penalty_rule4()
    }

    /// Rule 1: Runs of 5+ same-color modules in rows and columns.
    fn penalty_rule1(&self) -> u32 {
        let mut penalty = 0u32;

        // Rows
        for row in 0..self.size {
            let mut run = 1u32;
            for col in 1..self.size {
                if self.modules[row][col] == self.modules[row][col - 1] {
                    run += 1;
                } else {
                    if run >= 5 {
                        penalty += run - 2;
                    }
                    run = 1;
                }
            }
            if run >= 5 {
                penalty += run - 2;
            }
        }

        // Columns
        for col in 0..self.size {
            let mut run = 1u32;
            for row in 1..self.size {
                if self.modules[row][col] == self.modules[row - 1][col] {
                    run += 1;
                } else {
                    if run >= 5 {
                        penalty += run - 2;
                    }
                    run = 1;
                }
            }
            if run >= 5 {
                penalty += run - 2;
            }
        }

        penalty
    }

    /// Rule 2: 2x2 blocks of same color.
    fn penalty_rule2(&self) -> u32 {
        let mut penalty = 0u32;
        for row in 0..self.size - 1 {
            for col in 0..self.size - 1 {
                let c = self.modules[row][col];
                if c == self.modules[row][col + 1]
                    && c == self.modules[row + 1][col]
                    && c == self.modules[row + 1][col + 1]
                {
                    penalty += 3;
                }
            }
        }
        penalty
    }

    /// Rule 3: Pattern 10111010000 or 00001011101 in rows and columns.
    fn penalty_rule3(&self) -> u32 {
        let pattern_a: [bool; 11] = [
            true, false, true, true, true, false, true, false, false, false, false,
        ];
        let pattern_b: [bool; 11] = [
            false, false, false, false, true, false, true, true, true, false, true,
        ];

        let mut penalty = 0u32;

        // Rows
        for row in 0..self.size {
            if self.size >= 11 {
                for col in 0..=self.size - 11 {
                    let mut match_a = true;
                    let mut match_b = true;
                    for k in 0..11 {
                        if self.modules[row][col + k] != pattern_a[k] {
                            match_a = false;
                        }
                        if self.modules[row][col + k] != pattern_b[k] {
                            match_b = false;
                        }
                        if !match_a && !match_b {
                            break;
                        }
                    }
                    if match_a || match_b {
                        penalty += 40;
                    }
                }
            }
        }

        // Columns
        for col in 0..self.size {
            if self.size >= 11 {
                for row in 0..=self.size - 11 {
                    let mut match_a = true;
                    let mut match_b = true;
                    for k in 0..11 {
                        if self.modules[row + k][col] != pattern_a[k] {
                            match_a = false;
                        }
                        if self.modules[row + k][col] != pattern_b[k] {
                            match_b = false;
                        }
                        if !match_a && !match_b {
                            break;
                        }
                    }
                    if match_a || match_b {
                        penalty += 40;
                    }
                }
            }
        }

        penalty
    }

    /// Rule 4: Proportion of dark modules.
    fn penalty_rule4(&self) -> u32 {
        let total = (self.size * self.size) as u32;
        let mut dark = 0u32;
        for row in 0..self.size {
            for col in 0..self.size {
                if self.modules[row][col] {
                    dark += 1;
                }
            }
        }

        let percent = (dark * 100) / total;
        let lower = percent - (percent % 5);
        let upper = lower + 5;

        let penalty_lower = if lower >= 50 {
            (lower - 50) / 5
        } else {
            (50 - lower) / 5
        };
        let penalty_upper = if upper >= 50 {
            (upper - 50) / 5
        } else {
            (50 - upper) / 5
        };

        let min_penalty = if penalty_lower < penalty_upper {
            penalty_lower
        } else {
            penalty_upper
        };

        min_penalty * 10
    }

    // -----------------------------------------------------------------------
    // Format & Version Info
    // -----------------------------------------------------------------------

    fn write_format_info(&mut self, mask: u8) {
        let bits = tables::FORMAT_INFO_BITS_M[mask as usize];
        let s = self.size;

        // Location 1: around top-left finder
        for i in 0..6 {
            let bit = ((bits >> i) & 1) == 1;
            self.modules[8][i] = bit;
        }
        // Bit 6: row 8, col 7
        self.modules[8][7] = ((bits >> 6) & 1) == 1;
        // Bit 7: row 8, col 8
        self.modules[8][8] = ((bits >> 7) & 1) == 1;
        // Bit 8: row 7, col 8
        self.modules[7][8] = ((bits >> 8) & 1) == 1;
        // Bits 9-14: rows 5 down to 0, col 8
        for i in 0..6 {
            let bit = ((bits >> (9 + i)) & 1) == 1;
            self.modules[5 - i][8] = bit;
        }

        // Location 2: bottom-left and top-right
        // Bits 0-7: rows (size-1) down to (size-8), col 8
        for i in 0..8 {
            let bit = ((bits >> i) & 1) == 1;
            self.modules[s - 1 - i][8] = bit;
        }
        // Bits 8-14: row 8, cols (size-7) to (size-1)
        for i in 0..7 {
            let bit = ((bits >> (8 + i)) & 1) == 1;
            self.modules[8][s - 7 + i] = bit;
        }
    }

    fn write_version_info(&mut self) {
        if let Some(info) = tables::version_info(self.version) {
            let s = self.size;

            // 18 bits, placed in 6x3 blocks
            for i in 0..18 {
                let bit = ((info >> i) & 1) == 1;
                let row = i / 3;
                let col = i % 3;

                // Bottom-left: rows (s-11) to (s-9), cols 0 to 5
                self.modules[s - 11 + col][row] = bit;
                // Top-right: cols (s-11) to (s-9), rows 0 to 5
                self.modules[row][s - 11 + col] = bit;
            }
        }
    }
}
