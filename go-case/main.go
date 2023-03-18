package main

import (
	"fmt"
	"net/http"
	"encoding/json"
	"errors"

	spinredis "github.com/fermyon/spin/sdk/go/redis"
	spinhttp "github.com/fermyon/spin/sdk/go/http"
	spinconfig "github.com/fermyon/spin/sdk/go/config"
)

func init() {
	spinhttp.Handle(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			w.WriteHeader(405)
		} else if r.URL.Path == "/" {
			var v []string
			err := json.NewDecoder(r.Body).Decode(&v)
			if err == nil {
				dispatch(w, v)
			} else {
				w.WriteHeader(500)
			}
		} else if r.URL.Path != "/foo" {
			w.WriteHeader(404)
		} else if len(r.Header) != 1 || r.Header["Foo"][0] != "bar" {
			w.WriteHeader(400)
		} else {
			w.WriteHeader(200)
			w.Header().Set("lorem", "ipsum")
			fmt.Fprint(w, "dolor sit amet")
		}
	})

	spinredis.Handle(func(payload []byte) error {
		return nil
	})
}

func dispatch(w http.ResponseWriter, v []string) {
	err := execute(v)
	if err == nil {
		w.WriteHeader(200)
	} else {
		w.WriteHeader(500)
		fmt.Fprintln(w, err)
	}
}

func execute(v []string) error {
	if v[0] == "config" {
		spinconfig.Get(v[1])
		return nil
	} else {
		return errors.New(fmt.Sprintf("command not yet supported: %f", v[0]))
	}
}

func main() {}
