`timescale 1ns / 1ps
`default_nettype none

module eth_scrambler #(
            parameter int   NEW_PARAM = 32,
            parameter logic DATA_WIDTH = 32
        ) (
            input wire                      i_clk
        );
    
    (* MARK_DEBUG = "TRUE" *) parameter int Test;

    logic [2:0] te;

    logic       someth = 1;
    logic       someth;

    logic test;

    function void test_fn();
    endfunction

    typedef enum logic [1:0] {ENUM_ITEM, ENUM_ITEM_2} enum_t;

    enum_t enum_state;

    assign t = test[DATA_WIDTH];
    assign t = test[DATA_WIDTH_P];   
    assign t = test[variable_notdef];
    assign t = test[someth];   
    //comment
    
    /*bl
    * ock_commen
    *
    *
    *
    * t**/

`ifdef TEST
    module_m #(.PAR(DATA_WIDTH))
    module_u (.i_clk(i_clk));
`endif

    always_latch begin end
    always_ff @(posedge i_clk or negedge i_rst_n) begin : named_always_ff
        if (!i_rst_n) begin
            test <= 1'b1;
        end else begin
            test <= ~test;
        end
    end : named_always_ff

    always_ff @(posedge i_clk or negedge i_rst_n) begin : named_always_ff
        if (!i_rst_n) begin
            test <= 1'b1;
        end else begin
            case(enum_state)
                ENUM_ITEM: test <= ~test;
                ENUM_ITEM_2, ENUM_ITEM_3: begin
                    test <= ~test;
                end
                default: $display("Something");
            endcase
        end
    end : named_always_ff
    

    always_comb begin 
        someth = ~not_declared;
        not_declared = ~not_declared;
        not_declared = someth;
    end


endmodule

`resetall
