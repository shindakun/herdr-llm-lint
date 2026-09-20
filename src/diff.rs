/// The number of change hunks between two line sequences: maximal runs of
/// lines outside the longest common subsequence.
pub fn hunks(a: &[String], b: &[String]) -> usize {
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut count = 0;
    let mut in_hunk = false;
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            in_hunk = false;
            i += 1;
            j += 1;
            continue;
        }
        if !in_hunk {
            count += 1;
            in_hunk = true;
        }
        if j >= m || (i < n && lcs[i + 1][j] >= lcs[i][j + 1]) {
            i += 1;
        } else {
            j += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    #[test]
    fn counts_hunks() {
        assert_eq!(hunks(&l("a\nb\nc"), &l("a\nb\nc")), 0);
        assert_eq!(hunks(&l("a\nb\nc"), &l("a\nx\nc")), 1);
        assert_eq!(hunks(&l("a\nb\nc\nd\ne"), &l("a\nx\nc\nd\ny")), 2);
        assert_eq!(hunks(&l("a\nb"), &l("a\nb\nc\nd")), 1);
        assert_eq!(hunks(&l(""), &l("a")), 1);
        assert_eq!(hunks(&l("a\nb\nc"), &l("c")), 1);
    }
}
