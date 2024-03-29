#include <stdio.h>
#include "kdeconnectjb.h"

int main(int argc, char *argv[], char *envp[]) {
	@autoreleasepool {
		printf("Hello world!\n");
        printf("Rust interop: %lu\n", add(1, 1));
		return 0;
	}
}
