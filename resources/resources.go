package resources

import (
	"bytes"
	"fmt"
	"io"
	"net/http"
	"strings"
)

//go:generate go-bindata -pkg resources -ignore bindata\.go ./...

// AssetString returns an asset as a string.
func AssetString(name string) (string, error) {
	bytes, err := Asset(name)
	if err != nil {
		return "", err
	}
	return string(bytes), nil
}

// GetReader returns an asset as a reader.
func GetReader(name string) (io.Reader, error) {
	data, err := Asset(name)
	if err != nil {
		return nil, err
	}
	return bytes.NewReader(data), nil
}

type FileServer struct {
}

func (s FileServer) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	var path string

	if r.URL.String() == "/" {
		path = "index.html"
	} else {
		path = strings.TrimPrefix(r.URL.String(), "/")
	}

	asset := fmt.Sprintf("public/%s", path)
	bytes, err := Asset(asset)
	if err != nil {
		w.WriteHeader(http.StatusNotFound)
	} else {
		w.Write(bytes)
	}
}
