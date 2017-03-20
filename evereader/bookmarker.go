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
	"crypto/md5"
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/log"
	"os"
)

type Bookmark struct {
	// The Filename.
	Path string `json:"path"`

	// The offset, for Eve this is the line number.
	Offset uint64 `json:"offset"`

	// The sile of the file referenced in path.
	Size int64 `json:"size"`

	Sys interface{} `json:"sys"`
}

type Bookmarker struct {
	Filename string
	Reader   *FollowingReader
}

func NewBookmarker(reader *FollowingReader, directory string) (*Bookmarker, error) {
	var bookmarkFilename string

	if directory == "" {
		bookmarkFilename = fmt.Sprintf("%s.bookmark", reader.filename)
	} else {
		hash := md5.Sum([]byte(reader.filename))
		bookmarkFilename = fmt.Sprintf("%s/%x.bookmark",
			directory, hash)
	}

	bookmarker := &Bookmarker{
		Filename: bookmarkFilename,
		Reader:   reader,
	}
	if err := bookmarker.Init(true); err != nil {
		return nil, err
	}
	return bookmarker, nil
}

// GetBookmark returns a bookmark for the readers current location.
func (b *Bookmarker) GetBookmark() *Bookmark {
	bookmark := Bookmark{}
	bookmark.Path = b.Reader.path
	bookmark.Offset = b.Reader.Pos()

	fileInfo, err := b.Reader.Stat()
	if err == nil {
		bookmark.Sys = GetSys(fileInfo)
		bookmark.Size = fileInfo.Size()
	}

	return &bookmark
}

func (b *Bookmarker) WriteBookmark(bookmark *Bookmark) error {
	file, err := os.Create(b.Filename)
	if err != nil {
		return err
	}
	encoder := json.NewEncoder(file)
	err = encoder.Encode(bookmark)
	if err != nil {
		return err
	}
	return nil
}

func (b *Bookmarker) ReadBookmark() (*Bookmark, error) {
	file, err := os.Open(b.Filename)
	if err != nil {
		return nil, err
	}
	decoder := json.NewDecoder(file)
	decoder.UseNumber()
	var bookmark Bookmark
	err = decoder.Decode(&bookmark)
	if err != nil {
		return nil, err
	}
	return &bookmark, nil
}

func (b *Bookmarker) UpdateBookmark() error {
	bookmark := b.GetBookmark()
	return b.WriteBookmark(bookmark)
}

func (b *Bookmarker) BookmarkIsValid(bookmark *Bookmark) bool {

	if bookmark.Path != b.Reader.path {
		return false
	}

	fileInfo, err := b.Reader.Stat()
	if err == nil {

		// If the current file size is less than the bookmark file
		// size it was likely truncated, invalidate.
		if fileInfo.Size() < bookmark.Size {
			return false
		}

		if !SameSys(bookmark.Sys, GetSys(fileInfo)) {
			log.Debug("Current file does not matched bookmarked inode")
			return false
		}
	}

	return true
}

func (b *Bookmarker) Init(end bool) error {
	bookmark, err := b.ReadBookmark()

	if err == nil && b.BookmarkIsValid(bookmark) {
		err = b.Reader.SkipTo(bookmark.Offset)
		if err != nil {
			log.Error("Failed to skip to line %d, will skip to end of file: %s", err)
			b.Reader.SkipToEnd()
		}
	} else {
		if err != nil {
			log.Info("Failed to read bookmark: %s", err)
		} else {
			log.Info("Stale bookmark found")
		}
		if end {
			log.Info("Will start reading at end of file.")
			b.Reader.SkipToEnd()
		} else {
			log.Info("Will start reading at beginning of file.")
		}
	}

	// Test write.
	bookmark = b.GetBookmark()
	return b.WriteBookmark(bookmark)
}
