/* Copyright (c) 2016 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

package evereader

import (
	"bufio"
	"github.com/jasonish/evebox/eve"
	"io"
	"os"
)

type BasicReader struct {
	filename string
	file     *os.File
	reader   *bufio.Reader
}

func NewBasicReader(filename string) (*BasicReader, error) {
	file, err := os.Open(filename)
	if err != nil {
		return nil, err
	}

	basicReader := &BasicReader{
		filename: filename,
		file:     file,
		reader:   bufio.NewReader(file),
	}

	return basicReader, nil
}

func (r *BasicReader) Close() {
	r.file.Close()
}

func (r *BasicReader) NextLine() ([]byte, error) {

	offset, err := r.FileOffset()
	if err != nil {
		return nil, err
	}

	line, err := r.reader.ReadBytes('\n')
	if err != nil {
		if err == io.EOF && len(line) > 0 {
			// Data was read but hit EOF before end of line. Reset
			// the pointer and return EOF.
			r.SetOffset(offset)
		}
		return nil, err
	}

	return line, nil
}

func (r *BasicReader) Next() (eve.EveEvent, error) {
	line, err := r.NextLine()
	if err != nil {
		return nil, err
	}

	return eve.NewEveEventFromBytes(line)
}

func (r *BasicReader) FileSize() (int64, error) {
	info, err := r.file.Stat()
	if err != nil {
		return 0, err
	}
	return info.Size(), nil
}

// FileOffset returns the current read position from the beginning of file
// in bytes.
func (r *BasicReader) FileOffset() (int64, error) {
	return r.file.Seek(0, 1)
}

// SetOffset sets the file pointer to the current offset from the beginning of the
// file.
func (r *BasicReader) SetOffset(offset int64) {
	r.file.Seek(offset, 0)
}

func (r *BasicReader) Stat() (os.FileInfo, error) {
	return r.file.Stat()
}
