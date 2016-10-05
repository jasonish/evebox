// +build windows

package evereader

// The argument should actually be os.FileInfo, but that doesn't seem valid
// on Windows??  Doesn't matter, its not used anyways.
func GetSys(_ interface{}) interface{} {
	return map[string]interface{}{
	}
}

func SameSys(a interface{}, b interface{}) bool {
	return true
}