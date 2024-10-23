#include pre.mc

SET D3 12
#print_mem D3 
add D3 D3
#print_mem D3 

SET D3 102
push D3
#print_mem D2 D3
pop D2
#print_mem D2 D3