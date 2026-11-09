#include "bindings/bindings.h"
#include "Native/CoffiBridge.h"

int main(int argc, char * argv[]) {
	// Audio session, lock-screen controls and Siri must be wired up before the Rust core starts.
	coffi_native_install();
	ffi::start_app();
	return 0;
}
