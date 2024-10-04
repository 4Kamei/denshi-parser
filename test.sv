`timescale 1ns / 1ps
`default_nettype none

module eth_scrambler #(
        parameter int DATA_WIDTH = 32,
        parameter int NEW_PARAM = 32
        ) (
        input wire                      i_clk, 
        input wire                      i_rst_n,
        input wire                      i_scrambler_bypass,

        input  wire                     i_ready,
        output wire                     o_ready,
        
        input  wire                     i_valid,
        output wire                     o_valid,

        input  wire [DATA_WIDTH-1:0]    i_data,
        output wire [DATA_WIDTH-1:0]    o_data
    );

    logic [57:0]            scrambler;
    logic [DATA_WIDTH-1:0]  output_data;

    assign o_data       = output_data;

    logic [57:0]            new_scrambler_state;
    logic [DATA_WIDTH-1:0]  output_data_comb;

    
    assign o_ready = i_ready;   //Need to pass through ready
    
    logic valid_q;
    assign o_valid = valid_q;

    always_ff @(posedge i_clk or negedge i_rst_n) begin
        if (!i_rst_n) begin
            scrambler <= 58'h03FF_FFFF_FFFF_FFFF;    //2 ^ 58 - 1. Initalise to all ones 
            valid_q <= 1'b0;
        end else begin
            if ( i_valid && o_ready) begin
                scrambler <= new_scrambler_state;        
                output_data <= i_scrambler_bypass ? i_data : output_data_comb;
                valid_q <= 1'b1;
            end
            if (!i_valid && o_ready) begin
                valid_q <= 1'b0;
            end
        end
    end
    
    //The optimistic approach
    generate
        always_comb begin
            new_scrambler_state = scrambler;
            for (int i = 0; i < DATA_WIDTH; i++) begin
                output_data_comb[DATA_WIDTH-i-1] = i_data[DATA_WIDTH-i-1] ^ new_scrambler_state[38] ^ new_scrambler_state[57]; 
                new_scrambler_state = {new_scrambler_state[56:0], output_data_comb[DATA_WIDTH-i-1]};
            end
        end
    endgenerate

endmodule

`resetall
