#include pre.mc

SET D3 12
#print_mem D3
add D3 D3
#print_mem D3

SET D3 102
#print_mem PC D1 D2 D3 TMP COPY_TMP SP LR
push D3
push D3
