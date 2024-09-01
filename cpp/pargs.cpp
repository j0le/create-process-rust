#include <print>
#include <Windows.h>

int main(int argc, char**argv){
	std::println("ACP:        {:>10d}", GetACP());
	std::println("OEM CP:     {:>10d}", GetOEMCP());
	std::println("Console CP: {:>10d}", GetConsoleCP());
	for(int i = 0; i < argc; ++i) {
		std::print("arg {}: \"{}\", bytes: ", i, argv[i]);
		for(char*c = argv[i]; *c != '\0'; ++c){
			std::print("{:02x} ", *c);
		}
		std::println("");
	}
}
