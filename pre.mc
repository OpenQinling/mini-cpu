PC = 0x00 ; program counter, set to `0xf000` as default
D1 = 0x02
D2 = 0x04
D3 = 0x06
D4 = 0x08
; more registers...

SP      = 0xee ; stack pointer. set on push/pop
TO_CP 		= 0xf0 ; to impl mov, set on mov
CP = 0xfc

ZERO 	= 0xfe ; unset (0)
TO_PC   = ZERO

SET TO_CP CP
mov a b =
	STR a TO_CP
	LOD b TO_CP

not a =
    mov a D1
    SET a 0xFFFF
    SUB a D1

add a b =
    mov b  D1
    SET D2 0xFFFF
    SUB D2 D1     ; D2 = not D1
    SUB a  D2     ; a - (0xffff - b) = a + b - 0xffff = a + b + 1
    SET D1 0x01   ; D1 = 1
    SUB a  D1     ; a + b -1 -1 = a + b

jmp to =
    mov TO_PC to

; cond in {0, 1}
jne cond to = 	  ; if !cond { jmp to }
	mov cond D1   ; if cond == 1 => D1 = 1
	add D1 D1     ; if cond == 1 => d1 == 2
	add D1 D1     ; if cond == 1 => d1 == 4
	add D1 cond   ; if cond == 1 => d1 == 5
	add D1 D1     ; if cond == 1 => d1 == 10
	add D1 D1     ; if cond == 1 => PC += 5
	mov to PC     ; jmp to


SET SP 0xf000
push x =
	SET D1 2
	add D1 SP ; D1 = 2 + SP
	STR x  SP ; mem[SP] = x
	mov D1 SP

pop x =
	SET D1 2
	SUB SP D1
	LOD x  SP