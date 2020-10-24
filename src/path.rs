// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

pub fn expand(path: &str) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut paths = Vec::new();
    for path in glob::glob(path)? {
        if let Ok(path) = path {
            paths.push(path);
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod test {
    use super::expand;

    #[test]
    fn test_expand() {
        let paths = expand("src/*.rs").unwrap();
        assert!(paths.len() > 0);
    }
}
