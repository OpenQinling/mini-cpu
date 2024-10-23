; program counter, the value will be set when the program is loaded
; PC set to `0xf000` as default now
PC = 0x00
D1 = 0x02
D2 = 0x04
D3 = 0x06
D4 = 0x08
; more registers...



; constants for faster running
c0x0002 = 0xe4 ; const 0x0002
SET c0x0002 0x0002
c0xfffe = 0xe6 ; const 0xfffe(-2)
SET c0xfffe 0xfffe
c0xfffc = 0xe8 ; const 0xfffc(-4)
SET c0xfffc 0xfffc
c0xffff = 0xea ; const 0xffff
SET c0xffff 0xffff
c0x0001	= 0xec ; const 0x0001
SET c0x0001 0x0001

SP      = 0xee ; stack pointer
SET SP 0xe000

CP = 0xfc
TO_CP 	= 0xf0 ; to impl mov
SET TO_CP CP

ZERO 	= 0xfe ; unset (0)
TO_PC   = ZERO

mov a b =
	STR a TO_CP
	LOD b TO_CP

not a =
    mov a D1
    SET a 0xFFFF
    SUB a D1

add a b = ; a-(0xffff-b)-1 = a+b-0xffff-1 = a+b+1-1
    SET D1 0xFFFF  ; D1 = 0xffff
	STR b TO_CP    ; CP = b
    SUB D1 CP      ; D1 = 0xffff - b
    SUB a  D1      ; a  = a - D1 = a - (0xffff - b) = a + b + 1
    SUB a  c0x0001 ; a + b - 1 

jmp to =
    mov TO_PC to

; cond in {0, 1}
jne cond to = 	  ; if !cond { jmp to }
	SET CP    0xFFFF ; code from `add`, d1 = -1
	SUB CP    cond   ; CP = -2(cond=1) or -1(cond=0)
	SUB CP    CP     ; CP = -4(cond=1) or -2(cond=0)
	SUB CP    CP     ; CP = -8(cond=1) or -4(cond=0) 
	SUB CP    cond   ; CP = -9(cond=1) or -4(cond=0) delta=5
	SUB CP    0xfffc ; CP = -5(cond=1) or  0(cond-0)
	SUB CP 	  CP 	 ; PC+=  0(cond=1) or  5(cond=0)
	STR TO    TO_PC


push x =
	STR x  SP ; mem[SP] = x
	SUB SP c0xfffe ; SP -= -2

pop x =
	SUB SP c0x0002
	LOD x  SP