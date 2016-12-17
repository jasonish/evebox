package server

import (
	"encoding/json"
	"github.com/jasonish/evebox/log"
	"io"
	"net/http"
)

func SubmitHandler(appContext AppContext, r *http.Request) interface{} {

	count := uint64(0)

	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()

	for {
		var event map[string]interface{}

		err := decoder.Decode(&event)
		if err != nil {
			if err == io.EOF {
				break
			}
			log.Println(err)
			return err
		}

		count++
	}

	return count
}
