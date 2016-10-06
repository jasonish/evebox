package evereader

import (
	"os"
	"encoding/json"
	"github.com/jasonish/evebox/log"
)

type Bookmark struct {
	// The filename.
	Path   string `json:"path"`

	// The offset, for Eve this is the line number.
	Offset uint64 `json:"offset"`

	// The sile of the file referenced in path.
	Size   int64 `json:"size"`

	Sys    interface{} `json:"sys"`
}

type Bookmarker struct {
	Filename string
	Reader   *EveReader
}

func (b *Bookmarker) GetBookmark() (*Bookmark) {
	bookmark := Bookmark{}
	bookmark.Path = b.Reader.path
	bookmark.Offset = b.Reader.Pos()

	fileInfo, err := b.Reader.GetFileInfo()
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

func (b *Bookmarker) BookmarkIsValid(bookmark *Bookmark) bool {

	if bookmark.Path != b.Reader.path {
		return false;
	}

	fileInfo, err := b.Reader.GetFileInfo()
	if err == nil {

		// If the current file size is less than the bookmark file
		// size it was likely truncated, invalidate.
		if fileInfo.Size() < bookmark.Size {
			return false
		}

		if !SameSys(bookmark.Sys, GetSys(fileInfo)) {
			log.Error("Inodes don't match")
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
		log.Info("Failed to read bookmark: %s", err)
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