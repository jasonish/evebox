package evereader

import (
	"os"
	"encoding/json"
	"syscall"
)

type Bookmark struct {
	// The filename.
	Path   string `json:"path"`

	// The offset, for Eve this is the line number.
	Offset uint64 `json:"offset"`

	// The sile of the file referenced in path.
	Size   int64 `json:"size"`

	State  struct {
		       Inode uint64 `json:"inode"`
	       } `json:"state"`
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
		sysStat, ok := fileInfo.Sys().(*syscall.Stat_t)
		if ok {
			bookmark.State.Inode = sysStat.Ino
		}
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

		sysStat, ok := fileInfo.Sys().(*syscall.Stat_t)
		if ok {
			if sysStat.Ino != bookmark.State.Inode {
				return false
			}
		}
	}

	return true
}
